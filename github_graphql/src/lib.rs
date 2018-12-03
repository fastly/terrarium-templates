//! GraphQL example using GitHub's API.
//!
//! Adapted from the [`graphql_client`
//! example](https://github.com/graphql-rust/graphql-client/tree/master/graphql_client/examples/github/).

#![feature(proc_macro_hygiene)]
#[macro_use]
extern crate failure;
extern crate graphql_client;
#[macro_use]
extern crate http_guest;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate maud;
extern crate regex;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use failure::{Error, ResultExt};
use graphql_client::GraphQLQuery;
use maud::{Markup, DOCTYPE};
use regex::Regex;

use http_guest::{header, Request, RequestExt, Response};

/// Fill in this constant with a GitHub API token, following the instructions at this page:
/// <https://help.github.com/articles/creating-a-personal-access-token-for-the-command-line/>
///
/// We recommend that you give the token no privileged permissions; it will still be able to view
/// public repositories.
const GITHUB_API_TOKEN: &'static str = "YOUR_TOKEN_HERE";

type URI = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/schema.graphql",
    query_path = "src/query_1.graphql",
    response_derives = "Debug",
)]
struct RepoView;

/// The root page just has a link to the cranelift status page.
fn root_page() -> Markup {
    html! {
        div {
            h1 { "Repo Viewer" }
            a href="/view/CraneStation/cranelift" class="button primary" style="margin-top: 20px;" {
                "View CraneStation/cranelift"
            }
        }
    }
}

/// The `/view/$owner/$name` path handler gets the number of repo stars and a summary of recent
/// issues.
fn repo_info(owner: &str, name: &str) -> Result<Markup, Error> {
    let q = RepoView::build_query(repo_view::Variables {
        owner: owner.to_string(),
        name: name.to_string(),
    });
    let q_body = serde_json::to_vec(&q).context("query serialization")?;

    let query_resp = Request::builder()
        .method("POST")
        .uri("https://api.github.com/graphql")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", GITHUB_API_TOKEN),
        ).body(q_body)
        .context("query request construction")?
        .send()
        .context("query request sending")?;

    let query_resp_body: graphql_client::Response<repo_view::ResponseData> =
        serde_json::from_slice(query_resp.body()).context("query response deserialization")?;

    if let Some(errs) = query_resp_body.errors {
        Err(format_err!("GraphQL response errors: {:?}", errs))?
    }

    let query_resp_data: repo_view::ResponseData =
        query_resp_body.data.expect("query response data exists");

    let stars: Option<i64> = query_resp_data
        .repository
        .as_ref()
        .map(|repo| repo.stargazers.total_count);

    let issues = &query_resp_data
        .repository
        .expect("missing repository")
        .issues
        .nodes
        .expect("issue nodes is null");

    Ok(html! {
        div {
            h1 { (owner) "/" (name) " - 🌟 " (stars.unwrap_or(0)) }
            table class="fui-table" {
                tr {
                    th { "Title" }
                    th { "Comments" }
                }
                @for issue in issues {
                    @match issue {
                        Some(i) => {
                            tr {
                                td { (i.title) }
                                td { (i.comments.total_count) }
                            }
                        }
                        None => {}
                    }
                }
            }
        }
    })
}

fn server(req: &Request<Vec<u8>>) -> Result<Response<Vec<u8>>, Error> {
    // The page dispatch uses this regular expression to extract stations from the
    // /view/$owner/$name URL.
    lazy_static! {
        static ref RE: Regex = Regex::new("/view/([^/]+)/([^/]+)").expect("create regex");
    }
    let page_markup = if GITHUB_API_TOKEN == "YOUR_TOKEN_HERE" {
        html! {
            div {
                p { "Error: must set a valid GitHub API token before compiling." }
            }
        }
    } else {
        match RE.captures(req.uri().path()) {
            None if req.uri().path() == "/" => {
                // For requests to /, just dispatch to the root page:
                root_page()
            }
            Some(captures) => {
                // The regexp captures groups tell us the
                let owner = captures
                    .get(1)
                    .ok_or_else(|| format_err!("could not determine repo owner"))?
                    .as_str();
                let name = captures
                    .get(2)
                    .ok_or_else(|| format_err!("could not determine repo name"))?
                    .as_str();
                // Dispatch to the compare page:
                repo_info(owner, name)?
            }
            _ => {
                // Return early with a 404
                return Ok(Response::builder().status(404).body(vec![]).unwrap());
            }
        }
    };

    // Put the markup into the boilerplate, and render into a string
    let body = html! {
            (DOCTYPE)
            head {
                link rel="stylesheet" type="text/css" href="/style.css" {}
            }
            body {
                div class="fui-tile" style="display: flex;flex-direction: row; align-items: center; margin: 20px;" {
                    (page_markup)
                }
            }
    }.into_string();

    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/html; charset=utf-8")
        .body(body.as_bytes().to_owned())?)
}

fn user_entrypoint(req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    match server(req) {
        Ok(resp) => resp,
        Err(e) => {
            let body = format!("GitHub GraphQL demo error: {:?}", e);
            Response::builder()
                .status(500)
                .body(body.as_bytes().to_owned())
                .unwrap()
        }
    }
}

guest_app!(user_entrypoint);
