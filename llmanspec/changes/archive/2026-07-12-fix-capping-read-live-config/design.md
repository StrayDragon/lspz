# Design: fix-capping-read-live-config

CappingInterceptor { config: Arc<RwLock<Config>> }
intercept → read().capping.*
