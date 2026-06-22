use std::{cell::RefCell, collections::HashMap};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::{FluxError, Result},
    utils::memoize_property_getter,
};

fn normalize_slashes(path: &str) -> String {
    let mut result = String::new();
    let mut last_was_slash = false;
    for c in path.chars() {
        if c == '/' {
            if !last_was_slash {
                result.push(c);
                last_was_slash = true;
            }
        } else {
            result.push(c);
            last_was_slash = false;
        }
    }
    result
}

fn get_system_hostname() -> String {
    let mut buf = vec![0u8; 256];
    unsafe {
        if libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) == 0 {
            if let Some(idx) = buf.iter().position(|&b| b == 0) {
                buf.truncate(idx);
            }
            String::from_utf8_lossy(&buf).into_owned()
        } else {
            "localhost".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseUri {
    pub uri: String,
    pub scheme: String,
    pub netloc: String,
    pub path: String,
    pub query: String,
    pub query_dict: HashMap<String, Vec<String>>,
    pub fragment: String,
}

impl BaseUri {
    pub fn new(raw_uri: &str) -> Result<Self> {
        let parsed = Url::parse(raw_uri)?;
        let scheme = parsed.scheme().to_string();
        let netloc = parsed.authority().to_string();
        let path = parsed.path().to_string();
        let query = parsed.query().unwrap_or("").to_string();
        let fragment = parsed.fragment().unwrap_or("").to_string();

        let mut query_dict: HashMap<String, Vec<String>> = HashMap::new();
        for (key, val) in parsed.query_pairs() {
            query_dict
                .entry(key.into_owned())
                .or_default()
                .push(val.into_owned());
        }
        Ok(Self {
            uri: raw_uri.to_string(),
            scheme,
            netloc,
            path,
            query,
            query_dict,
            fragment,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobUri {
    pub base: BaseUri,
    pub remote_hostname: Option<String>,
    remote_uri: RefCell<Option<String>>,
    local_uri: RefCell<Option<String>>,
}

impl JobUri {
    pub fn new(uri: &str, remote_hostname: Option<String>) -> Result<Self> {
        let mut base = BaseUri::new(uri)?;
        if base.scheme.is_empty() {
            return Err(FluxError::Logic(format!(
                "JobURI '{}' does not have a valid scheme",
                uri
            )));
        }
        base.path = normalize_slashes(&base.path);
        Ok(Self {
            base,
            remote_hostname,
            remote_uri: RefCell::new(None),
            local_uri: RefCell::new(None),
        })
    }

    memoize_property_getter!(
        #[memoized_property(remote_uri, String, Clone)]
        pub fn as_remote(&self) {
            {
                match self.base.scheme.as_str() {
                    "ssh" => self.base.uri.clone(),
                    "local" => {
                        let hostname = self
                            .remote_hostname
                            .clone()
                            .unwrap_or_else(get_system_hostname);
                        format!("ssh://{}{}", hostname, self.base.path)
                    }
                    _ => {
                        return Err(FluxError::Logic(format!(
                            "Cannot convert JobURI with scheme {} to remote",
                            self.base.scheme
                        )))
                    }
                }
            }
        }
    );
    // pub fn as_remote(&self) -> Result<String> {
    //     let mut remote_uri_cache = self.remote_uri.borrow_mut();
    //     if remote_uri_cache.is_none() {
    //         let resolved = match self.base.scheme.as_str() {
    //             "ssh" => self.base.uri.clone(),
    //             "local" => {
    //                 let hostname = self
    //                     .remote_hostname
    //                     .clone()
    //                     .unwrap_or_else(get_system_hostname);
    //                 format!("ssh://{}{}", hostname, self.base.path)
    //             }
    //             _ => {
    //                 return Err(FluxError::Logic(format!(
    //                     "Cannot convert JobURI with scheme {} to remote",
    //                     self.base.scheme
    //                 )))
    //             }
    //         };
    //         *remote_uri_cache = Some(resolved);
    //     }
    //     // Using 'unwrap()' here is safe since we just ensured there is a value in the option above
    //     Ok(remote_uri_cache.as_ref().unwrap().clone())
    // }

    pub fn as_local(&self) -> Result<String> {
        let mut local_uri_cache = self.local_uri.borrow_mut();
        if local_uri_cache.is_none() {
            let resolved = match self.base.scheme.as_str() {
                "ssh" => format!("local://{}", self.base.path),
                "local" => self.base.uri.clone(),
                _ => {
                    return Err(FluxError::Logic(format!(
                        "Cannot convert JobURI with scheme {} to local",
                        self.base.scheme
                    )))
                }
            };
            *local_uri_cache = Some(resolved);
        }
        // Using 'unwrap()' here is safe since we just ensured there is a value in the option above
        Ok(local_uri_cache.as_ref().unwrap().clone())
    }
}
