#![feature(proc_macro_hygiene)]
#[macro_use]
extern crate lazy_static;
extern crate http;
extern crate regex;
#[macro_use]
extern crate http_guest;
#[macro_use]
extern crate failure;
extern crate htmlescape;
extern crate serde_json;
#[macro_use]
extern crate maud;

use failure::Error;
use http_guest::{PendingRequest, Request, RequestExt, Response};
use maud::{Markup, PreEscaped, DOCTYPE};
use regex::Regex;
use serde_json::Value;

///! This web application fetches weather observations from the weather.gov JSON API,
///! and renders an HTML page.
///! For the weather API documentation, see:
///! https://www.weather.gov/documentation/services-web-api#/default/get_stations__stationId__observations_latest

/// A WeatherRequest is a request made to the weather API.
struct WeatherRequest {
    location: String,
    pending: PendingRequest,
}

impl WeatherRequest {
    /// Initiate a request. `location` is a friendly, human-readable name; `station_code`
    /// is the name of the weather station, typically an ICAO airport code in the US,
    /// e.g. KPDX or KSFO
    pub fn new(location: &str, station_code: &str) -> Result<WeatherRequest, Error> {
        // Build a request:
        let req = Request::builder()
            .method("GET")
            .header("accept", "application/ld+json")
            .uri(format!(
                "https://api.weather.gov/stations/{}/observations/latest",
                station_code
            )).body(vec![])?;
        // Send it asynchronously:
        let pending = req.send_async()?;
        Ok(WeatherRequest {
            location: location.to_owned(),
            pending,
        })
    }
    /// Get the response corresponding to the request. The request was sent asynchronously
    /// in `new`, this will block until a response is returned.
    pub fn get_response(self) -> Result<WeatherObservation, Error> {
        // Block for the response
        let resp = self.pending.wait()?;
        // Construct a `WeatherObservation` if successful, otherwise show the error:
        if resp.status() == 200 {
            WeatherObservation::new(self.location, resp)
        } else {
            let err_body = std::str::from_utf8(resp.body()).unwrap_or("(utf8 error)");
            Err(format_err!(
                "weather request for {} returned {}: {}",
                self.location,
                resp.status(),
                err_body,
            ))
        }
    }
}

/// A WeatherObservation is built from a response from the weather API
struct WeatherObservation {
    /// Location of the weather observation station.
    pub location: String,
    /// Textual description of the current weather.
    pub description: String,
    /// Current temperature in degrees celsius.
    pub celsius: f64,
}

impl WeatherObservation {
    /// Construct a WeatherObservation from an API response.
    /// The (friendly, human-readable) location isnt stored in the response, we
    /// get that from the caller.
    pub fn new(location: String, response: Response<Vec<u8>>) -> Result<Self, Error> {
        // Decode the response body into a JSON structure:
        let resp_body: &[u8] = &response.body();
        let json: Value = serde_json::de::from_slice(resp_body)?;

        // .textDescription should be a text field. Extract it, and change it
        // to lowercase.
        let description = json
            .get("textDescription")
            .ok_or(format_err!("weather missing textDescription"))?
            .as_str()
            .ok_or(format_err!("textDescription was not str"))?
            .to_lowercase();

        // .temperature.value should be a numeric field. Extract it as a f64:
        let celsius = json
            .get("temperature")
            .ok_or(format_err!("weather missing temperature"))?
            .get("value")
            .ok_or(format_err!("temperature missing value"))?
            .as_f64()
            .ok_or(format_err!("temperature value is not a number"))?;

        Ok(WeatherObservation {
            location,
            description,
            celsius,
        })
    }

    /// Provide the temperature in fahrenheit
    pub fn fahrenheit(&self) -> f64 {
        self.celsius * 9.0 / 5.0 + 32.0
    }

    /// Text description of the weather:
    pub fn description(&self) -> String {
        format!(
            "The weather in {} is currently {} and {}&deg;F ({}&deg;C)",
            htmlescape::encode_minimal(&self.location),
            htmlescape::encode_minimal(&self.description),
            self.fahrenheit() as i32,
            self.celsius as i32
        )
    }
}

/// The root page makes a single weather API request to show the current weather in Portland
fn root_page() -> Result<Markup, Error> {
    // Start a request to get the weather at pdx, and wait for the response:
    let pdx = WeatherRequest::new("Portland, OR", "KPDX")?.get_response()?;

    // Create maud Markup for the page
    Ok(html! {
        div {
            h1 { "Current Weather" }
            p { (PreEscaped(pdx.description())) }
            a href="/compare/KPDX/to/KSFO" class="button primary" style="margin-top: 20px;" {
                "Compare to SF"
            }
        }
    })
}

/// The compare page makes two simultaneous weather API requests, to compare the weather at two
/// different observation stations.
fn compare_page(station1: &str, station2: &str) -> Result<Markup, Error> {
    // Send request for the weather at the two stations to compare.
    // We don't have a simple human-readable name for these stations. Maybe you could add one?
    let req1 = WeatherRequest::new(station1, station1)?;
    let req2 = WeatherRequest::new(station2, station2)?;

    // Now, wait for both responses:
    let resp1 = req1.get_response()?;
    let resp2 = req2.get_response()?;

    // Create maud Markup for the page
    Ok(html! {
        div {
            h1 { "Weather Comparison" }
            p { (PreEscaped(resp1.description())) }
            p { (PreEscaped(resp2.description())) }
        }
    })
}

/// Implementation of the application server
fn server(req: &Request<Vec<u8>>) -> Result<Response<Vec<u8>>, Error> {
    // The page dispatch uses this regular expression to extract stations from the
    // /compare/<station1>/to/<station2> URL.
    lazy_static! {
        static ref RE: Regex =
            Regex::new("/compare/([[:alpha:]]+)/to/([[:alpha:]]+)").expect("create regex");
    }
    let page_markup = match RE.captures(req.uri().path()) {
        None if req.uri().path() == "/" => {
            // For requests to /, just dispatch to the root page:
            root_page()
        }
        Some(captures) => {
            // The regexp captures groups tell us the
            let station1 = captures
                .get(1)
                .ok_or_else(|| format_err!("could not determine request station 1"))?
                .as_str();
            let station2 = captures
                .get(2)
                .ok_or_else(|| format_err!("could not determine request station 2"))?
                .as_str();
            // Dispatch to the compare page:
            compare_page(station1, station2)
        }
        _ => {
            // Return early with a 404
            return Ok(Response::builder().status(404).body(vec![]).unwrap());
        }
    }?;
    // Put the markup into the boilerplate, and render into a string
    let body = html! {
            (DOCTYPE)
            head {
                link rel="stylesheet" type="text/css" href="/style.css" {}
            }
            body {
                div class="fui-tile" style="display: flex;flex-direction: row; align-items: center; margin: 20px;" {
                    img
                        src="/weather.jpg"
                        style="max-height: 200px; margin-top: 20px; margin-bottom: 20px"
                        {}
                (page_markup)
                }
            }
    }.into_string();
    // Send the string as a successful response.
    let resp = Response::builder()
        .status(200)
        .header("content-type", "text/html; charset=utf-8")
        .body(body.as_bytes().to_owned())?;
    Ok(resp)
}

/// Wrapper for the server that turns the error case into a 500 response showing
/// the error:
fn server_(req: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    match server(req) {
        Ok(resp) => resp,
        Err(e) => {
            let body = format!("Weather JSON API Demo Error: {:?}", e);
            Response::builder()
                .status(500)
                .body(body.as_bytes().to_owned())
                .unwrap()
        }
    }
}
/// Macro that sets server_ as the entry point of the guest application:
guest_app!(server_);
