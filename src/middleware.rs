use actix_web::body::{to_bytes, MessageBody};
use actix_web::{dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, web, Error, HttpResponse};
use actix_web::HttpMessage;
use futures_util::future::LocalBoxFuture;
use futures_util::{FutureExt, StreamExt};
use log::{debug, info, warn};
use std::future::{ready, Ready};
use std::io::Bytes;

/// 日志中间件结构体
pub struct Logging;

/// 实现中间件工厂的Transform trait
/// S - 下一个服务的类型
/// B - 响应体的类型
impl<S, B> Transform<S, ServiceRequest> for Logging
where
    S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = LoggingMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    /// 创建新的中间件实例
    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(LoggingMiddleware { service }))
    }
}

/// 日志中间件实现结构体
pub struct LoggingMiddleware<S> {
    service: S,
}

/// 实现Service trait来处理请求和响应
impl<S, B> Service<ServiceRequest> for LoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    // 转发ready!宏到内部服务
    forward_ready!(service);

    /// 处理请求并记录日志
    fn call(&self, req: ServiceRequest) -> Self::Future {
        // 获取请求信息
        let path = req.path().to_string();
        let headers = req.headers().to_owned();
        let query_params = req.query_string().to_string();
        
        // 记录请求日志
        debug!("request path: {}, \nheader:{:?}, \nquery_params:{} ",
            path, headers, query_params);

        // 调用内部服务处理请求
        let future = self.service.call(req);

        // 处理响应并记录日志
        Box::pin(async move {
            let res: ServiceResponse<B> = future.await?;

            // 记录响应状态和头部信息
            let status = res.status();
            let headers = res.headers().clone();
            debug!("Status Code: {}", status);
            debug!("Headers: {:?}", headers);
            Ok(res)
        })
    }
}