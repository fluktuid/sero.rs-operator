use serde::{Deserialize, Serialize};
use thiserror::Error;
use anyhow::bail;
use tracing::warn;

#[derive(Error, Debug)]
pub enum FormatError {
    #[error("Missing attribute: {0}")]
    MissingAttribute(String),
}

#[derive(Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct SeroConfig {
    // Lots of complicated fields.
    pub image: String,
    pub service: String,
    pub service_inject: bool,
    pub deployment: String,
    pub timeout_forward_ms: i64,
    pub timeout_scale_up_ms: i64,
    pub timeout_scale_down_ms: i64,
}

impl SeroConfig {
    // This method will help users to discover the builder
    pub fn builder() -> SeroConfigBuilder {
      SeroConfigBuilder::default()
    }

    pub fn name_patern(&self) -> String {
        format!("sero-{}", self.deployment)
    }
}

impl Default for SeroConfig {
    fn default() -> SeroConfig {
        SeroConfig {
            image: String::from("ghcr.io/fluktuid/sero.rs"),
            service_inject: true,
            timeout_forward_ms: 2000,
            timeout_scale_up_ms: 5000,
            timeout_scale_down_ms: 15000,
            service: String::new(),
            deployment: String::new(),
        }
    }
}

#[derive(Default)]
pub struct SeroConfigBuilder {
    // Probably lots of optional fields.
    image: Option<String>,
    service: Option<String>,
    service_inject: bool,
    deployment: Option<String>,
    timeout_forward_ms: i64,
    timeout_scale_up_ms: i64,
    timeout_scale_down_ms: i64,
}

impl SeroConfigBuilder {
    pub fn new() -> SeroConfigBuilder {
        // Set the minimally required fields of Foo.
        SeroConfigBuilder {
            service: None,
            deployment: None,
            ..Default::default()
        }
    }

    pub fn service(mut self, service: String) -> SeroConfigBuilder {
        self.service = Some(service); self
    }

    pub fn inject(mut self, inject: bool) -> SeroConfigBuilder {
        self.service_inject = inject; self
    }

    pub fn image(mut self, image: String) -> SeroConfigBuilder {
        self.image = Some(image); self
    }

    pub fn deployment(mut self, deployment: String) -> SeroConfigBuilder {
        self.deployment = Some(deployment); self
    }

    pub fn timeout_forward(mut self, millis: i64) -> SeroConfigBuilder {
        self.timeout_forward_ms = millis; self
    }

    pub fn timeout_scale_up(mut self, millis: i64) -> SeroConfigBuilder {
        self.timeout_scale_up_ms = millis; self
    }

    pub fn timeout_scale_down(mut self, millis: i64) -> SeroConfigBuilder {
        self.timeout_scale_down_ms = millis; self
    }

    pub fn build(self) -> Result<SeroConfig, anyhow::Error> {
        if self.deployment.is_none() {
            bail!("Missing attribute: deployment")
        }
        if self.service.is_none() {
            warn!("Missing attribute: service. Using deployment name.")
        }
        let deploy = self.deployment.unwrap();
        Ok(SeroConfig {
            service: self.service.unwrap_or(deploy.clone()),
            service_inject: self.service_inject,
            deployment: deploy,
            timeout_forward_ms: self.timeout_forward_ms,
            timeout_scale_up_ms: self.timeout_scale_up_ms,
            timeout_scale_down_ms: self.timeout_scale_down_ms,
            ..Default::default()
        })
    }
}
