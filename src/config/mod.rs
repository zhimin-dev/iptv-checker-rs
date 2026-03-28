// 导出config模块

// 任务配置模块（核心配置）
pub mod task;

// 搜索源配置模块
pub mod search;

// 字符串替换配置模块
pub mod replace;

// 收藏夹配置模块
pub mod favourite;

// Logo配置模块
pub mod logos;

// Base配置模块
pub mod base;

// EPG配置模块
pub mod epg;

// 导出file_config模块中的所有内容
pub use task::file_config::*;

/// 初始化所有配置文件
/// 
/// 此函数会创建所有必需的配置文件（如果它们不存在）：
/// - core/task.json - 任务管理配置
/// - core/search.json - 搜索源配置
/// - core/replace.json - 字符串替换配置
/// - core/favourite.json - 收藏夹配置
/// - core/logos.json - Logo配置
/// - core/base.json - Base配置
/// - core/epg.json - EPG配置
pub fn init_all_config_files() {
    task::init_task_config();
    search::create_search_file();
    replace::create_replace_file();
    favourite::create_favourite_file();
    logos::create_logos_file();
    base::create_base_file();
    epg::create_epg_file();
    // 兼容：若 logos.json 有 host 且 base.json 为空，则同步到 base.json
    base::sync_host_from_logos_if_needed();
}
