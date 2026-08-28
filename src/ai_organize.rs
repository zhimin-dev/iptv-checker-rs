//! AI 整理：把频道名称发给 DeepSeek，返回规范化的
//! name / aliases / tvg_id / group1 / group2，并支持合并进频道图标统一配置。

use futures::StreamExt;
use serde::{Deserialize, Serialize};

/// 单个 AI 整理结果（分组为「分组映射」的扁平分组）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiChannelItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tvg_id: String,
    #[serde(default)]
    pub group: String,
}

/// DeepSeek 返回的 choices 结构（仅取需要的字段）
#[derive(Debug, Deserialize)]
struct DeepSeekResp {
    choices: Vec<DeepSeekChoice>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChoice {
    message: DeepSeekMessage,
}

#[derive(Debug, Deserialize)]
struct DeepSeekMessage {
    content: String,
}



fn system_prompt(allow_create_groups: bool, group_mode: &str) -> String {
    let group_rule = if !allow_create_groups {
        "4. group：频道所属分组，必须从用户提供的分组列表中选择一个（如 央视、卫视、地方台）；没有合适分组就留空字符串。"
    } else if group_mode == "category" {
        "4. group：频道所属分组，按电视分类归类（如 央视、卫视、地方台、少儿、体育、新闻、影视、纪录片 等）。如果用户提供的分组列表中有合适的就从中选择，否则创建新的分组名（使用简洁的常见分类）。确实无法判断时才留空字符串。"
    } else {
        // 默认：前缀/地域分组
        "4. group：频道所属分组。优先按频道名前缀归类（如 CCTV、CCTV1、CCTV5 等全部归入央视）；前缀不一致时按省份/地域归类（如 东方卫视→上海、浙江卫视→浙江、湖南卫视→湖南；跨地域或无法判断地域的全国性频道按 卫视/央视 等归并）。如果用户提供的分组列表中有合适的就从中选择，否则创建新的分组名（使用简洁常见命名）。确实无法判断时才留空字符串。"
    };
    format!(
        r#"你是 IPTV 频道信息整理助手。用户会给出：1) 用户已设置的分组列表；2) 频道名称列表（含 EPG 名称与频道图标名称，名称可能写法不同但含义相同）。

请按含义把名称合并分组，每个含义输出一条：
1. name：该含义下最标准的中文频道名（去除画质/格式后缀如 720p、1080p、4K、HD、HEVC，去除【】（）() [] 等括号内容）。
2. aliases：该含义下的常见名称变体（把用户给出的相同含义、不同写法的名称放进 aliases，如「CCTV-1 综合」「CCTV1综合」「东方卫视【台】」），最多 4 个，保持精简。
3. tvg_id：EPG 节目单匹配 id（如 CCTV1、CCTV5）；不确定就留空字符串 ""。
{}

只输出一个 JSON 对象，格式：
{{"items":[{{"name":"...","aliases":["..."],"tvg_id":"...","group":"..."}}]}}
不要输出任何其它文字。"#,
        group_rule
    )
}

/// 调用 DeepSeek 整理一批频道名（names 为原始频道名列表，groups 为分组映射的扁平分组）
async fn call_deepseek(
    config: &crate::config::ai::AiConfig,
    names: &[String],
    groups: &[String],
    allow_create_groups: bool,
    group_mode: &str,
) -> Result<Vec<AiChannelItem>, String> {
    let url = format!("{}/chat/completions", config.base_url);
    let mut lines: Vec<String> = Vec::new();
    lines.push("用户已设置的分组列表（每行一个）：".to_string());
    for g in groups {
        let g = g.trim();
        if !g.is_empty() {
            lines.push(g.to_string());
        }
    }
    lines.push(String::new());
    lines.push("频道名称列表（每行一个）：".to_string());
    for (i, n) in names.iter().enumerate() {
        lines.push(format!("{}. {}", i + 1, n));
    }
    let user_content = lines.join("\n");
    let body = serde_json::json!({
        "model": config.model,
        "messages": [
            { "role": "system", "content": system_prompt(allow_create_groups, group_mode) },
            { "role": "user", "content": user_content }
        ],
        "response_format": { "type": "json_object" },
        "temperature": 0.1,
        "stream": false,
    });
    // DeepSeek 国内可直连：用直连客户端（不走系统代理），避免 Clash 节点不稳导致长时间卡住
    let client = crate::common::util::get_direct_http_client();
    let mut last_err = String::new();
    let mut resp = None;
    for _attempt in 0..2 {
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(300))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => {
                resp = Some(r);
                break;
            }
            Err(e) => {
                last_err = format!("请求 DeepSeek 失败: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
    let resp = resp.ok_or(last_err)?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    if !status.is_success() {
        return Err(format!("DeepSeek 返回 {}: {}", status, text.chars().take(300).collect::<String>()));
    }
    let parsed: DeepSeekResp =
        serde_json::from_str(&text).map_err(|e| format!("解析 DeepSeek 响应失败: {}", e))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "DeepSeek 响应为空".to_string())?;
    // 兼容 content 中可能包裹了 markdown 代码块
    let fence_open = String::from("```json");
    let fence = String::from("```");
    let json_str = content
        .trim()
        .trim_start_matches(fence_open.as_str())
        .trim_start_matches(fence.as_str())
        .trim_end_matches(fence.as_str())
        .trim();
    let obj: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("解析 AI 输出 JSON 失败: {}", e))?;
    let items: Vec<AiChannelItem> = obj
        .get("items")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(items)
}

/// 整理频道名称：分块调用 DeepSeek，返回所有结果与错误列表
pub async fn organize_channel_names(
    names: Vec<String>,
    groups: Vec<String>,
    allow_create_groups: bool,
    group_mode: String,
) -> Result<(Vec<AiChannelItem>, Vec<String>), String> {
    let config = crate::config::ai::get_ai_config();
    if config.api_key.trim().is_empty() {
        return Err("未配置 DeepSeek API Key，请先到「设置」中保存 API Key".to_string());
    }
    let names: Vec<String> = names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        return Err("没有可整理的频道名称".to_string());
    }
    // 单次请求内部分批并行调用（每批 40 个、3 路并发），大幅缩短总耗时
    const BATCH_SIZE: usize = 40;
    const CONCURRENCY: usize = 3;
    let mut results: Vec<(usize, Result<Vec<AiChannelItem>, String>)> = futures::stream::iter(
        names
            .chunks(BATCH_SIZE)
            .enumerate()
            .map(|(idx, chunk)| {
                let config = config.clone();
                let groups = groups.clone();
                let group_mode = group_mode.clone();
                async move {
                    let r = call_deepseek(&config, chunk, &groups, allow_create_groups, &group_mode).await;
                    (idx, r)
                }
            }),
    )
    .buffer_unordered(CONCURRENCY)
    .collect()
    .await;
    results.sort_by_key(|(idx, _)| *idx);
    let mut items: Vec<AiChannelItem> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for (_idx, r) in results {
        match r {
            Ok(mut list) => items.append(&mut list),
            Err(e) => errors.push(e),
        }
    }
    Ok((items, errors))
}

/// 把 AI 整理结果应用：
/// - 别名 / tvg-id 合并进频道图标统一配置（保留图标，匹配不到则新建）；
/// - 分组写入「分组映射」（tvg-name → 分组，仅接受用户已设置的分组名）。
/// 返回 (更新数, 新建数, 分组映射数)。
pub fn apply_ai_items(
    items: Vec<AiChannelItem>,
    allow_create_groups: bool,
) -> Result<(usize, usize, usize), String> {
    let mut config = crate::config::channel_icons::get_channel_icons();
    let mut updated = 0usize;
    let mut created = 0usize;
    let mut grouped = 0usize;
    let allowed_groups: std::collections::HashSet<String> =
        crate::config::group::get_groups().into_iter().collect();

    for item in items {
        let name = item.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        // 该含义下的全部名称（标准名 + 别名）
        let mut meaning_names: Vec<String> = Vec::new();
        meaning_names.push(name.clone());
        for a in &item.aliases {
            let a = a.trim().to_string();
            if !a.is_empty() && !meaning_names.contains(&a) {
                meaning_names.push(a);
            }
        }
        let meaning_norms: Vec<String> = meaning_names
            .iter()
            .map(|n| crate::epg_mapping::normalize_epg_name(n))
            .filter(|n| !n.is_empty())
            .collect();
        // 匹配已有配置项：标准名或别名任一命中
        let mut matched = false;
        for existing in config.items.iter_mut() {
            let hit = {
                let own_norm = crate::epg_mapping::normalize_epg_name(&existing.name);
                meaning_norms.contains(&own_norm)
                    || existing.aliases.iter().any(|a| {
                        !a.trim().is_empty() && meaning_norms.contains(&crate::epg_mapping::normalize_epg_name(a))
                    })
            };
            if !hit {
                continue;
            }
            matched = true;
            // 别名取并集（把 EPG 名称变体补充进 aliases）
            for n in &meaning_names {
                if !existing.aliases.iter().any(|a| a.trim().eq_ignore_ascii_case(n.trim()))
                    && !existing.name.trim().eq_ignore_ascii_case(n.trim())
                {
                    existing.aliases.push(n.clone());
                }
            }
            if existing.tvg_id.trim().is_empty() && !item.tvg_id.trim().is_empty() {
                existing.tvg_id = item.tvg_id.trim().to_string();
            }
            updated += 1;
        }
        if !matched {
            config.items.push(crate::config::channel_icons::ChannelIconItem {
                name,
                aliases: item.aliases,
                tvg_id: item.tvg_id.trim().to_string(),
                group1: String::new(),
                group2: String::new(),
                group: String::new(),
                logo: String::new(),
            });
            created += 1;
        }
        // 分组写入分组映射：默认仅接受用户已设置的分组名；
        // 开启「可创建分组」时允许 AI 新建分组（set_group_mapping 会自动加入分组列表）
        let group = item.group.trim().to_string();
        if !group.is_empty() && (allow_create_groups || allowed_groups.contains(&group)) {
            for n in &meaning_names {
                if crate::config::group::set_group_mapping(n.clone(), group.clone()).is_ok() {
                    grouped += 1;
                }
            }
        }
    }
    crate::config::channel_icons::save_channel_icons(config.items)?;
    Ok((updated, created, grouped))
}
