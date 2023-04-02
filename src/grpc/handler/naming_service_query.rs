#![allow(unused_imports)]

use crate::{
    grpc::{PayloadHandler, 
        api_model::{SUCCESS_CODE, ERROR_CODE, Instance as ApiInstance,ServiceInfo as ApiServiceInfo, ServiceQueryRequest, ServiceQueryResponse}, 
        nacos_proto::Payload, PayloadUtils
    }, 
    naming::{core::{NamingActor, NamingCmd, NamingResult}, 
    model::{Instance, ServiceKey, ServiceInfo}, NamingUtils}, 
    now_millis_i64
};
use actix::prelude::Addr;
use async_trait::async_trait;

use super::converter::ModelConverter;

pub struct ServiceQueryRequestHandler{
    naming_addr: Addr<NamingActor>,
}

impl ServiceQueryRequestHandler {
    pub fn new(naming_addr: Addr<NamingActor>) -> Self {
        Self { naming_addr }
    }

    fn convert_to_service_info(&self,info:ServiceInfo) -> ApiServiceInfo {
        ModelConverter::to_api_service_info(info)
    }

    fn parse_clusters(&self,request:&ServiceQueryRequest) -> Vec<String> {
        let mut clusters = vec![];
        if let Some(cluster_str) = request.cluster.as_ref() {
            clusters = cluster_str.split(",").into_iter()
                .filter(|e|{e.len()>0}).map(|e|{e.to_owned()}).collect::<Vec<_>>();
        }
        clusters
    }
}

#[async_trait]
impl PayloadHandler for ServiceQueryRequestHandler {
    async fn handle(&self, request_payload: crate::grpc::nacos_proto::Payload,_request_meta:crate::grpc::RequestMeta) -> anyhow::Result<Payload> {
        let body_vec = request_payload.body.unwrap_or_default().value;
        let request:ServiceQueryRequest = serde_json::from_slice(&body_vec)?;
        let mut response = ServiceQueryResponse::default();
        let clusters = self.parse_clusters(&request);
        let namespace = request.namespace.as_ref().unwrap_or(&"public".to_owned()).to_owned();
        let key = ServiceKey::new(&namespace
            ,&request.group_name.unwrap_or_default(),&request.service_name.unwrap_or_default());
        let cmd = NamingCmd::QueryServiceInfo(key,clusters,true);
        match self.naming_addr.send(cmd).await{
            Ok(res) => {
                let result: NamingResult = res.unwrap();
                match result {
                    NamingResult::ServiceInfo(service_info) => {
                        let api_service_info = self.convert_to_service_info(service_info);
                        response.service_info = Some(api_service_info);
                        response.result_code = SUCCESS_CODE;
                    },
                    _ => {
                        response.result_code = ERROR_CODE;
                        response.error_code = 500u16;
                        response.message = Some("naming handler result type is error".to_owned());
                    }
                };
            },
            Err(err) => {
                response.result_code = ERROR_CODE;
                response.error_code = 500u16;
                response.message = Some(err.to_string());
            }
        };
        Ok(PayloadUtils::build_payload("QueryServiceResponse", serde_json::to_string(&response)?))
    }

}