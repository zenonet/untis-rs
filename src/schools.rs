use embedded_svc::http::client::Connection;

use crate::{
    error::Error, jsonrpc, params::FindSchoolParams, resources::School, SchoolSearchResult,
};

fn get_client<C>(client: C) -> jsonrpc::Client<C>
where C: Connection {
    jsonrpc::Client::new("https://mobile.webuntis.com/ms/schoolquery2", client)
}

/// Returns all schools matching the query or an empty vec if there are too many results.
pub fn search<C>(query: &str, client: C) -> Result<Vec<School>, Error> 
where C: Connection {
    let result = get_client(client).request(
        "searchSchool",
        vec![FindSchoolParams::Search { search: query }],
    );
    catch_too_many(result)
}

/// Retrieves a school by its id.
pub fn get_by_id<C>(id: &usize, client: C) -> Result<School, Error> 
where C: Connection{
    let result = get_client(client).request(
        "searchSchool",
        vec![FindSchoolParams::ById { schoolid: id }],
    );

    get_first(catch_too_many(result)?)
}

/// Retrieves a school by it's [`login_name`](School#structfield.login_name).
pub fn get_by_name<C>(name: &str, client: C) -> Result<School, Error> 
where C: Connection{
    let result = get_client(client).request(
        "searchSchool",
        vec![FindSchoolParams::ByName { schoolname: name }],
    );

    get_first(catch_too_many(result)?)
}

fn get_first(mut list: Vec<School>) -> Result<School, Error> {
    if list.is_empty() {
        Err(Error::NotFound)
    } else {
        Ok(list.swap_remove(0))
    }
}

fn catch_too_many(result: Result<SchoolSearchResult, Error>) -> Result<Vec<School>, Error> {
    match result {
        Ok(v) => Ok(v.schools),
        Err(Error::Rpc(err)) => {
            if err.code == jsonrpc::ErrorCode::TooManyResults.as_isize() {
                Ok(vec![])
            } else {
                Err(Error::Rpc(err))
            }
        }
        Err(err) => Err(err),
    }
}
