use std::io::Cursor;

use ::bytes::{Buf, Bytes};
use anyhow::Result;
use flate2::bufread::GzDecoder;
use regex::*;
use serde::Deserialize;
use tar::Archive;

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

    pub async fn pull_layers(&mut self, image: &str) -> Result<()> {
        let image: Image = image.into();
        let manifest = self.get_manifest(&image).await?;

        for layer in manifest.fsLayers.into_iter() {
            let bytes = self.fetch_layer(&image.name, layer.blobSum).await?;
            self.unpack(bytes)?;
        }
        Ok(())
    }

    fn unpack(&self, bytes: Bytes) -> Result<()> {
        let tar = GzDecoder::new(Cursor::new(bytes).reader());
        let mut archive = Archive::new(tar);
        archive.unpack("/")?;
        Ok(())
    }

    async fn get_manifest(&mut self, image: &Image) -> Result<Manifest> {
        let mut resp = self.manifest_request(&image).send().await?;
        if resp.status().as_u16() == 401 {
            let info: AuthInfo = resp
                .headers()
                .get("WWW-Authenticate")
                .unwrap()
                .to_str()?
                .into();
            self.authorize(info).await?;
            resp = self.manifest_request(&image).send().await?;
        };
        let manifest = resp.json::<Manifest>().await?;
        Ok(manifest)
    }

    async fn fetch_layer(&self, name: &String, digest: String) -> Result<Bytes> {
        let resp = self
            .client
            .get(format!(
                "https://registry.hub.docker.com/v2/library/{}/blobs/{}",
                name, digest
            ))
            // Should have been set in manifest fetch
            .bearer_auth(self.token.as_ref().unwrap())
            .send()
            .await?;
        let bytes = resp.bytes().await?;
        Ok(bytes)
    }

    fn manifest_request(&self, image: &Image) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .get(format!(
                "https://registry.hub.docker.com/v2/library/{}/manifests/{}",
                image.name, image.reference
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

struct Image {
    name: String,
    reference: String,
}

impl From<&str> for Image {
    fn from(value: &str) -> Self {
        let mut split = value.split(":");
        Image {
            name: split.next().unwrap().to_string(),
            reference: split.next().get_or_insert("latest").to_string()
        }
    }
}
