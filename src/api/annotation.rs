use std::{collections::BTreeMap, sync::Arc};

use futures::{TryStreamExt};
use k8s_openapi::api::apps::v1::Deployment;
use kube::{runtime::watcher, Client, Api, api::ListParams, ResourceExt};
use tokio::{sync::{mpsc::Sender, RwLock}, task::JoinHandle};
use tracing::{debug, info, warn};

#[derive(Clone)]
pub enum State {
  Added,
  Modified,
  Deleted,
}
#[derive(Clone)]
pub enum AppType {
  Managed,
  SeroSelf,
  NotManaged,
}

#[derive(Clone)]
pub struct ChangeObject<T> {
  pub object: T,
  pub state: State,
}

#[derive(Clone)]
pub struct AnnotationWatcher {
  pub namespace: Vec<String>,
  handler: Arc<RwLock<BTreeMap<String, JoinHandle<()>>>>,
  tx: Sender<ChangeObject<Deployment>>,
}

impl AnnotationWatcher {
  pub fn new(tx: Sender<ChangeObject<Deployment>>) -> AnnotationWatcher {
    let t = AnnotationWatcher {
      namespace: vec![],
      handler: Arc::new(RwLock::new(BTreeMap::new())),
      tx: tx,
    };
    t
  }

  pub async fn remove_ns(&self, name: String) {
    match self.handler.read().await.get(&name) {
        Some(v) => {v.abort();},
        None => {},
    }
  }

  pub async fn add_ns(&mut self, namespace: String) {
    info!("spawn");
    // todo: implement first state check
    let tx = self.tx.clone();
    let ns = namespace.clone();
    // todo: handle JoinHandle
    let handler = tokio::spawn(async move {
      let client = match Client::try_default().await {
          Ok(v) => v,
          Err(e) => {warn!("failed creating client {}", e); return},
      };
      let deployments: Api<Deployment> = Api::namespaced(client, &namespace);
      info!("starting watcher for ns {}", namespace.clone());
      let watch = watcher(deployments, ListParams::default())
        .try_for_each(|e| async {
          debug!("got watch event");
          match e {
            watcher::Event::Applied(d) => {
              match get_type(d.annotations()) {
                AppType::SeroSelf => {
                  info!("add sero self: {}", d.clone().metadata.name.unwrap());},
                AppType::Managed => {
                  info!("add event: {}", d.clone().metadata.name.unwrap());
                  _ = tx.send(ChangeObject { object: d, state: State::Added }).await;
                },
                AppType::NotManaged => {_ = tx.send(ChangeObject { object: d, state: State::Deleted }).await;},
              }
            },
            watcher::Event::Deleted(d) => {
              match get_type(d.annotations()) {
                AppType::SeroSelf => {},
                _ => {_ = tx.send(ChangeObject { object: d, state: State::Deleted }).await;}
              }
            },
            // todo: implement ::Restarted
            _ => {},
          };
          Ok(())
        })
        ;
      match watch.await {
          Ok(_) => {},
          Err(e) => {warn!("err {}", e)},
      };
      info!("started watcher");
    });
    self.handler.write().await.insert(ns,handler);
  }
}

pub fn get_type(annotations: &BTreeMap<String, String>) -> AppType {
  let has_annotation = annotations.keys().any(|e| {
    e.contains("beta.v1.sero/")
  });
  let has_config = annotations.keys().any(|e| {
    e == &String::from("beta.v1.sero/config")
  });
  if has_annotation && has_config {
    return AppType::SeroSelf;
  } else if has_annotation && !has_config {
    return AppType::Managed;
  }
  return AppType::NotManaged;
}
