use std::collections::HashMap;

use crate::error;
use chrono::{DateTime, FixedOffset};
use embedded_svc::{http::client::Connection};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// Error codes contained in [Untis API errors](Error).
/// The underlying integer can be accessed using [code.as_isize()](Self::as_isize()).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum ErrorCode {
    UserBlocked = -8998,
    NotAuthenticated = -8520,
    NoAccess = -8509,
    InvalidCredentials = -8504,
    InvalidSchoolName = -8500,
    TooManyResults = -6003,
}

impl ErrorCode {
    pub fn as_isize(&self) -> isize {
        *self as isize
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub(crate) struct Request<'a, P: Serialize> {
    jsonrpc: &'static str,
    id: &'a str,
    method: &'a str,
    params: P,
}

impl<'a, P: Serialize> Request<'a, P> {
    pub fn new(id: &'a str, method: &'static str, params: P) -> Self {
        Self {
            id,
            method,
            jsonrpc: "2.0",
            params,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Response<T> {
    Ok {
        jsonrpc: String,
        id: serde_json::Value,
        result: T,
    },
    Err {
        jsonrpc: String,
        id: serde_json::Value,
        error: Error,
    },
}

/// JSON-RPC error object.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Error {
    pub code: isize,
    pub message: String,
}

pub(crate) struct Client<C> {
    http_client: embedded_svc::http::client::Client<C>,
    url: String,
    last_req_id: usize,
    pub session_id: Option<String>,
    pub date: Option<DateTime<FixedOffset>>
}

impl<C> Client<C>
where C: Connection {
    pub fn new(url: &str, client: C) -> Self {
        Self {
            http_client: embedded_svc::http::client::Client::wrap(client),
            url: url.to_string(),
            last_req_id: 0,
            session_id: None,
            date: None
        }
    }

    fn get_id(&mut self) -> String {
        self.last_req_id += 1;
        self.last_req_id.to_string()
    }

    pub fn request<T: DeserializeOwned, P: Serialize>(
        &mut self,
        method: &'static str,
        params: P,
    ) -> Result<T, error::Error> {
        let req_id = &self.get_id();
        let data = Request::new(req_id, method, params);
        
        //let cookie_header: String = build_cookie_header(&self.cookies).unwrap_or_else(|| String::new());
        //println!("Cookie header:\nCookie: {}", &cookie_header);

        let cookie_str = self.session_id.as_ref().and_then(|session| Some(format!("JSESSIONID={}", session))).unwrap_or_else(|| String::new());
        let cookies = [("Cookie", cookie_str.as_str())];

        let mut request= self.http_client.post(&self.url, &cookies[..]).unwrap(); 
    
        let buf = serde_json::to_vec(&data).unwrap();
        println!("Request: {}", &String::from_utf8(buf.clone()).unwrap());
        request.write(&buf).unwrap();
        let mut response = request.submit().unwrap();
    
        let status = response.status();
        if !status == 200 {
            return Err(error::Error::Http(status));
        }

        // Update time
        self.date = response.header("Date").and_then(|d| DateTime::parse_from_rfc2822(d).ok());

        // save cookies
/*         println!("Got set-cookie header: {:?}", &response.header("Set-Cookie"));
        collect_cookies(&response, &mut self.cookies);
        println!("Cookies collected:\n{:#?}", self.cookies); */



        let text = read_response(&mut response);
        println!("{}", text);
        let response: Response<T> = serde_json::from_str(&text)?;


        match response {
            Response::Ok {
                jsonrpc: _,
                id: _,
                result,
            } => Ok(result),

            Response::Err {
                jsonrpc: _,
                id: _,
                error,
            } => Err(error::Error::Rpc(error)),
        }
    }
}



fn read_response<C>(response: &mut embedded_svc::http::client::Response<&mut C>) -> String
where C: Connection{
    if let Some(len) = response.header("Content-Length"){
        let len = len.parse::<usize>().unwrap();
        let mut buf = vec![0u8; len];
        let _ = response.read(&mut buf);
        String::from_utf8(buf).unwrap()
    }else{
        let mut buf = Box::new(Vec::new());
        // Read in chunks; avoid using BufReader as requested.
        let mut tmp = [0u8; 1024];

        loop {
            match response.read(&mut tmp) {
                Ok(0) => break, // EOF — server closed connection, body complete
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
                _ => break,
            }
        }
        println!("Read {}", buf.len());
        let s = String::from_utf8(*buf).unwrap();
        println!("read:\n{s}");
        s
    }

}

fn parse_set_cookie(set_cookie: &str) -> Option<(String, String)> {
    // Split on ';' to separate cookie pair from attributes
    let mut parts = set_cookie.splitn(2, ';');
    let pair = parts.next()?.trim();
    let mut kv = pair.splitn(2, '=');
    let name = kv.next()?.trim();
    let value = kv.next().unwrap_or("").trim();
    if name.is_empty() {
        None
    } else {
        Some((name.to_string(), value.to_string()))
    }
}
pub fn collect_cookies<C>(resp: &embedded_svc::http::client::Response<C>, map: &mut HashMap<String, String>)
where C:Connection {
    if let Some(raw) = resp.header("Set-Cookie") {
        let candidates = raw.split(&['\r', '\n'][..]).filter(|s| !s.trim().is_empty());
        for sc in candidates {
            if let Some((k, v)) = parse_set_cookie(sc) {
                *map.entry(k).or_insert(String::new()) = v;
            }
        }
    }
}

pub fn build_cookie_header(cookies: &HashMap<String, String>) -> Option<String> {
    if cookies.is_empty() {
        return None;
    }
    let mut pairs: Vec<String> = Vec::with_capacity(cookies.len());
    for (k, v) in cookies {
        pairs.push(format!("{}={}", k, v));
    }
    Some(pairs.join("; "))
}