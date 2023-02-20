use anyhow::Result;
use regex::*;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    fsLayers: Vec<Layer>,
}

#[derive(Deserialize, Debug)]
struct Layer {
    blobSum: String,
}

pub struct ApiClient {
    client: reqwest::Client,
    token: Option<String>,
}

impl ApiClient {
    pub fn new() -> Self {
        ApiClient {
            client: reqwest::Client::new(),
            token: None,
        }
    }

    pub async fn get_manifest(&mut self, name: &str, reference: &str) -> Result<Manifest> {
        let mut resp = self.manifest_request(name, reference).send().await?;
        if resp.status().as_u16() == 401 {
            let info: AuthInfo = resp
                .headers()
                .get("WWW-Authenticate")
                .unwrap()
                .to_str()?
                .into();
            println!("{:?}", info);
            self.authorize(info).await?;
            resp = self.manifest_request(name, reference).send().await?;
        };
        let manifest = resp.json::<Manifest>().await?;
        println!("{:?}", manifest);
        Ok(manifest)
    }

    fn manifest_request(&self, name: &str, reference: &str) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .get(format!(
                "https://registry.hub.docker.com/v2/library/{}/manifests/{}",
                name, reference
            ))
            .header(
                "Accept",
                // "application/vnd.docker.distribution.manifest.v2+json",
                "application/vnd.oci.image.index.v1+json",
            );
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        req
    }

    async fn authorize(&mut self, info: AuthInfo) -> Result<()> {
        let resp = self
            .client
            .get(format!(
                "{}?service={}&scope={}",
                info.realm, info.service, info.scope
            ))
            .send()
            .await?;
        let auth = resp.json::<Authorization>().await?;
        self.token = Some(auth.token);
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct Authorization {
    token: String,
}

#[derive(Debug)]
struct AuthInfo {
    realm: String,
    service: String,
    scope: String,
}

impl From<&str> for AuthInfo {
    fn from(value: &str) -> Self {
        let re = Regex::new(r#"^Bearer realm="(.*)",service="(.*)",scope="(.*)"$"#).unwrap();
        match re.captures(value) {
            Some(m) => AuthInfo {
                realm: m.get(1).unwrap().as_str().to_string(),
                service: m.get(2).unwrap().as_str().to_string(),
                scope: m.get(3).unwrap().as_str().to_string(),
            },
            _ => panic!(""),
        }
    }
}
