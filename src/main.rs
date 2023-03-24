mod api;
use api::annotation::{AnnotationWatcher, self, ChangeObject};
use serde::{Serialize, de::DeserializeOwned};
use tokio::{sync::{mpsc, RwLock}};
use crate::api::namespace;
mod sero_config;
use k8s_openapi::{apimachinery::pkg::apis::meta::v1::LabelSelector, NamespaceResourceScope};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{DeleteParams, PostParams};
use kube::core::{ObjectMeta};
use k8s_openapi::api::apps::v1::{Deployment, DeploymentSpec};
use k8s_openapi::api::core::v1::{EnvFromSource, Container, ContainerPort, PodSpec, PodTemplateSpec,ConfigMapEnvSource, ConfigMap, Service, ServiceSpec, ServicePort};
use anyhow::{Result, Ok};
use anyhow::bail;
use sero_config::{SeroConfigBuilder, SeroConfig};
mod operator_config;
use operator_config::{Settings, DefaultSeroConfig};
use std::collections::BTreeMap;
use std::sync::Arc;
use kube::{api::Api, Client};
use tracing::{info, warn};
use annotation::State;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let settings = match Settings::new() {
        Result::Ok(v) => v,
        Err(e) => {
            warn!("{}",e);
            Settings::default()
        },
    };

    let (tx, mut rx) = mpsc::channel(16);
    let (ns_tx, mut ns_rx) = mpsc::channel::<ChangeObject<String>>(10);
    let ns = settings.namespaces;
    if ns.clone().len() == 0 {
        info!("No default NS List: creating ns watcher");
        let a_watch = AnnotationWatcher::new(tx);
        let a_watch = Arc::new(RwLock::new(a_watch));
        namespace::spawn(ns_tx).await;

        _ = tokio::spawn(async move {
            let s = a_watch.clone();
            for e in s.read().await.namespace.clone() {
            s.write().await.add_ns(e).await;
            }
            while let Some(co) = ns_rx.recv().await {
                let obj = co.object;
                info!("ns event {}", obj);
                match co.state {
                    State::Added => {_ = s.write().await.add_ns(obj.to_string()).await;},
                    State::Modified => { /* todo: implement */},
                    State::Deleted => {_ = s.read().await.remove_ns(obj.to_string()).await;},
                }
            };
        });
    } else {
        info!("Static List of watched namespaces");
        tokio::spawn(async move {
            for e in ns {_ = ns_tx.send(ChangeObject { object: e, state: State::Added }).await;}
        });
    }

    info!("created watcher");
    let anno = tokio::spawn(async move {
        while let Some(co) = rx.recv().await {
            let obj = co.object.clone();
            let meta = obj.metadata.clone();
            let default = settings.default_config.clone();
            let config = match po_to_cfg(obj, default) {
                Result::Ok(v) => v,
                Err(e) => {
                    warn!("error: {}", e);
                    continue;
                },
            };
            match co.state {
                State::Added => {info!("state added");_ = apply_sero_instance(&config).await;},
                State::Modified => {info!("state modified");_ = update_sero_instance(&config, meta).await;},
                State::Deleted => {
                    info!("state deleted");
                    let name = config.name_patern();
                    _ = remove_sero_instance(&name).await;
                },
            }
        };
    });

    anno.await?;

    Ok(())
}

fn po_to_cfg(data: Deployment, default: DefaultSeroConfig) -> Result<SeroConfig> {
    let annotations = match data.metadata.annotations.clone() {
        Some(v) => v,
        None => {bail!("no annotation on new service")},
    };
    match to_config(annotations, data.metadata.name.unwrap(), default) {
        Result::Ok(v) => Ok(v),
        Err(e) => return Err(e),
    }
}

async fn apply_sero_instance(sero_config: &SeroConfig) -> Result<()> {
    info!("Creating new Sero instance for deploy {}", sero_config.deployment);
    let sero_config_str = serde_json::to_string(&sero_config).unwrap();
    let some_name = Some(sero_config.name_patern());
    let name = sero_config.name_patern();
    let deployment = Deployment {
        metadata: ObjectMeta {
            name: some_name.clone(),
            annotations: Some(BTreeMap::from(
                [("beta.v1.sero/config".to_string(), sero_config_str.clone()),]
            )),
            // todo: add operator as ownerReference
            ..Default::default() 
        },
        spec: Some(DeploymentSpec {
            selector: LabelSelector {
                match_labels: Some(
                    BTreeMap::from([
                        (String::from("beta.v1.sero/deploy"), sero_config.deployment.clone()),
                        (String::from("beta.v1.sero/service"), sero_config.service.clone()),
                    ])),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    annotations: Some(
                        BTreeMap::from([
                            (String::from("beta.v1.sero/config"), sero_config_str.clone()),
                        ])
                    ),
                    labels: Some(
                        BTreeMap::from([
                            (String::from("beta.v1.sero/deploy"), sero_config.deployment.clone()),
                            (String::from("beta.v1.sero/service"), sero_config.service.clone()),
                        ])
                    ),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    automount_service_account_token: Some(true),
                    containers: vec![Container {
                        env_from: Some(vec![
                            EnvFromSource {
                                config_map_ref: Some(ConfigMapEnvSource {
                                    name: some_name.clone(),
                                    optional: Some(false),
                                }),
                                ..Default::default()
                            }
                        ]),
                        image: Some(sero_config.image.clone()),
                        name: String::from("sero"),
                        ports: Some(vec![ContainerPort {
                            container_port: 8080,
                            name: Some(String::from("tcp")),
                            protocol: Some(String::from("TCP")),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    };
    match create_or_update(&deployment, &name.clone()).await {
        Result::Ok(_) => {},
        Err(e) => {warn!("deploy {}", e)},
    };

    let configmap = ConfigMap {
        data: Some(
            BTreeMap::from([
                ("deployment".to_uppercase(), sero_config.deployment.clone()),
                ("service".to_uppercase(), sero_config.service.clone()),
                ("inject".to_uppercase(), sero_config.service_inject.to_string()),
                ("timeout_forward".to_uppercase(), sero_config.timeout_forward_ms.to_string()),
                ("timeout_scale_up".to_uppercase(), sero_config.timeout_scale_up_ms.to_string()),
                ("timeout_scale_down".to_uppercase(), sero_config.timeout_scale_down_ms.to_string()),
            ]),
        ),
        metadata: ObjectMeta {
            name: some_name.clone(),
            annotations: Some(
                BTreeMap::from([
                    (String::from("beta.v1.sero/config"), sero_config_str.clone()),
                ])
            ),
            labels: Some(
                BTreeMap::from([
                    (String::from("beta.v1.sero/deploy"), sero_config.deployment.clone()),
                    (String::from("beta.v1.sero/service"), sero_config.service.clone()),
                ])
            ),
            ..Default::default()
        },
        ..Default::default()
    };
    match create_or_update(&configmap, &name.clone()).await {
        Result::Ok(_) => {},
        Err(e) => {warn!("cm {}", e)},
    };

    let svc = Service {
        metadata: ObjectMeta {
            name: some_name.clone(),
            annotations: Some(
                BTreeMap::from([
                    (String::from("beta.v1.sero/config"), sero_config_str.clone()),
                ])
            ),
            labels: Some(
                BTreeMap::from([
                    (String::from("beta.v1.sero/deploy"), sero_config.deployment.clone()),
                    (String::from("beta.v1.sero/service"), sero_config.service.clone()),
                ])
            ),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some(
                BTreeMap::from([
                    (String::from("beta.v1.sero/deploy"), sero_config.deployment.clone()),
                    (String::from("beta.v1.sero/service"), sero_config.service.clone()),
                ])
            ),
            ports: Some(vec![ServicePort{
                name: Some(String::from("tcp")),
                port: 80,
                target_port: Some(IntOrString::Int(8080)),
                protocol: Some(String::from("TCP")),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };
    match create_or_update(&svc, &name.clone()).await {
        Result::Ok(_) => {},
        Err(e) => {warn!("svc {}", e)},
    };

    // todo: add rbac

    //todo: add operator as ownerReference to all objects
    Ok(())
}

async fn update_sero_instance(sero_config: &SeroConfig, om: ObjectMeta) -> Result<()> {
    let name = om.name.unwrap();
    let client = Client::try_default().await?;
    let deploy: Api<Deployment> = Api::default_namespaced(client);
    let metadata = deploy.get_metadata(&name).await.unwrap();
    match metadata.metadata.annotations {
        Some(a) => {
            match annotation::get_type(&a) {
                annotation::AppType::Managed => {
                    apply_sero_instance(sero_config).await?;
                    return Ok(());
                }
                _ => {},
            }
        },
        None => {},
    }
    let name = sero_config.name_patern();
    remove_sero_instance(&name).await?;
    Ok(())
}

async fn remove_sero_instance(name: &str) -> Result<()> {
    info!("removing Sero instance for {}", name);
    let client = Client::try_default().await?;
    let deploy: Api<Deployment> = Api::default_namespaced(client.clone());
    match deploy.delete(&name, &DeleteParams::background()).await {
        Result::Ok(_) => {},
        Err(e) => warn!("error deleting {}", e),
    };
    let cm: Api<ConfigMap> = Api::default_namespaced(client.clone());
    match cm.delete(&name, &DeleteParams::background()).await {
        Result::Ok(_) => {},
        Err(e) => warn!("error deleting {}", e),
    };
    let svc: Api<Service> = Api::default_namespaced(client.clone());
    match svc.delete(&name, &DeleteParams::background()).await {
        Result::Ok(_) => {},
        Err(e) => warn!("error deleting {}", e),
    };

    //todo: check for ownerReference in all objects

    // todo: add rbac
    Ok(())
}

fn to_config(annotations: BTreeMap<String, String>, name: String, default: DefaultSeroConfig) -> Result<SeroConfig> {
    let mut builder = SeroConfigBuilder::new()
        .deployment(name)
        .image(default.image)
        .inject(default.inject)
        //.protocol(default.protocol)
        //.port(default.port)
        .timeout_forward(default.timeout.forward_ms)
        .timeout_scale_up(default.timeout.scale_up_ms)
        .timeout_scale_down(default.timeout.scale_down_ms);
    for (k, v) in annotations.into_iter() {
        builder = match k.to_lowercase().as_str() {
            "beta.v1.sero/service" => {builder.service(v)}
            "beta.v1.sero/inject" => {match v.parse::<bool>() {
                Result::Ok(v) => {builder.inject(v)},
                Err(_) => {warn!("can't parse {}={}. Using default.", k,v); builder},
            }},
            //"beta.v1.sero/deployment" => {builder.deployment(v)},
            "beta.v1.sero/timeout-forward" => {match v.parse::<i64>() {
                Result::Ok(v) => {builder.timeout_forward(v)},
                Err(_) => {warn!("can't parse {}={}. Using default.", k,v); builder},
            }},
            "beta.v1.sero/timeout-scaleup" => {match v.parse::<i64>() {
                Result::Ok(v) => {builder.timeout_scale_up(v)},
                Err(_) => {warn!("can't parse {}={}. Using default.", k,v);builder},
            }},
            "beta.v1.sero/timeout-scale-down" => {match v.parse::<i64>() {
                Result::Ok(v) => {builder.timeout_scale_down(v)},
                Err(_) => {warn!("can't parse {}={}. Using default.", k,v);builder},
            }},
            _ => {builder},
        };
    }
    builder
        .build()
}

async fn create_or_update<T: kube::Resource>(t: &T, name: &str) -> Result<T, kube::Error> 
where
    <T as kube::Resource>::DynamicType: Default,
    T: kube::Resource<Scope = NamespaceResourceScope> + Clone + DeserializeOwned + Serialize + std::fmt::Debug,
{
    let client = Client::try_default().await?;
    let api: Api<T> = Api::<T>::default_namespaced(client.clone());

    let is = api.get_metadata_opt(name).await?;
    match is.is_some() {
        //true => api.patch(&name.clone(), &PatchParams::force(PatchParams::apply("sero")), &Patch::Apply(t)).await,
        true => api.replace(&name, &PostParams::default(), &t).await,
        false => api.create(&PostParams::default(), &t).await,
    }
}