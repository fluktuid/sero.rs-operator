use std::{collections::BTreeMap};

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Namespace;
use kube::{runtime::watcher, Client, Api, api::ListParams, ResourceExt};
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::annotation::{ChangeObject, State};

pub async fn spawn(tx: Sender<ChangeObject<String>>) {
  info!("spawn");
  // todo: implement first state check
  // todo: handle JoinHandle
  _ = tokio::spawn(async move {
    let client = Client::try_default().await.unwrap();
    let ns: Api<Namespace> = Api::all(client);
    info!("starting watcher");
    match ns.list_metadata(&ListParams::default()).await {
        Ok(e) => {
          for e in e.items.into_iter()
              .filter(|e| {e.annotations().contains_key("beta.v1.sero/scan")})
              .map(|e| {e.metadata.name.unwrap()}) {
            info!("send msg {}", e);
            let r = tx.send(ChangeObject { object: e, state: State::Added }).await;
            if r.is_err() {
              warn!("e: {}", r.unwrap_err());
            }
          }
        },
        Err(_) => {return;},
    };
    let watch = watcher(ns, ListParams::default())
      .try_for_each(|e| async {
        info!("got ns event");
        match e {
          watcher::Event::Applied(d) => {
            if has_sero_annotation(d.annotations()) {
              let name = d.metadata.name.unwrap();
              info!("add ns event: {}", name);
              _ = tx.send(ChangeObject { object: name, state: State::Added }).await;
            } else {
              _ = tx.send(ChangeObject { object: d.metadata.name.unwrap(), state: State::Deleted }).await;
            }
          },
          watcher::Event::Deleted(d) => {
            if has_sero_annotation(d.annotations()) {
              let name = d.metadata.name.unwrap();
              info!("remove ns event: {}", name);
              _ = tx.send(ChangeObject { object: name, state: State::Deleted }).await;
            }},
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
}

pub fn has_sero_annotation(annotations: &BTreeMap<String, String>) -> bool{
  annotations.keys().any(|e| {
    e.contains("sero/")
  })
}
