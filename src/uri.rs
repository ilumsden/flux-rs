use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{FluxError, Result};

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
    #[serde(skip)]
    pub(crate) remote_uri: RefCell<Option<String>>,
    #[serde(skip)]
    pub(crate) local_uri: RefCell<Option<String>>,
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

    pub fn as_remote(&self) -> Result<String> {
        let mut remote_uri_cache = self.remote_uri.borrow_mut();
        if remote_uri_cache.is_none() {
            let resolved = match self.base.scheme.as_str() {
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
            };
            *remote_uri_cache = Some(resolved);
        }
        // Using 'unwrap()' here is safe since we just ensured there is a value in the option above
        Ok(remote_uri_cache.as_ref().unwrap().clone())
    }

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

impl Display for JobUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let force_local = std::env::var("FLUX_URI_RESOLVE_LOCAL").is_ok();
        if force_local {
            match self.as_local() {
                Ok(local_str) => write!(f, "{}", local_str),
                Err(_) => write!(f, "{}", self.base.uri),
            }
        } else {
            write!(f, "{}", self.base.uri)
        }
    }
}

impl Deref for JobUri {
    type Target = BaseUri;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for JobUri {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Debug, Clone)]
pub struct UriResolverUri {
    pub base: BaseUri,
}

impl UriResolverUri {
    pub fn new(uri: &str) -> Result<Self> {
        let modified_uri = uri.replacen(':', ":FXX", 1);
        let mut base = BaseUri::new(&modified_uri)?;

        base.path = base.path.replacen("FXX", "", 1);

        Ok(Self { base })
    }
}

impl Deref for UriResolverUri {
    type Target = BaseUri;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UriResolverUri {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// TODO add URI resolvers
