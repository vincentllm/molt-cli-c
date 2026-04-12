pub mod direct_llm;
pub mod feishu;

use anyhow::Result;
use crate::config::BackendConfig;
use crate::recording::MarkSlice;

/// 所有 AI 提取后端必须实现的 trait
pub trait AiBackend {
    fn extract_pipeline(&self, slices: &[MarkSlice]) -> Result<String>;
}

/// 根据配置构建对应后端实例
pub fn build_backend(cfg: &BackendConfig) -> Box<dyn AiBackend> {
    match cfg {
        BackendConfig::DirectLlm { api_key, base_url, model } => {
            Box::new(direct_llm::DirectLlmBackend {
                api_key: api_key.clone(),
                base_url: base_url.clone(),
                model: model.clone(),
            })
        }
        BackendConfig::FeishuBot { app_id, app_secret, chat_id, poll_timeout_secs } => {
            Box::new(feishu::extractor::FeishuBotBackend {
                app_id: app_id.clone(),
                app_secret: app_secret.clone(),
                chat_id: chat_id.clone(),
                poll_timeout_secs: *poll_timeout_secs,
            })
        }
    }
}

/// 构造发给 AI 的提取 Prompt（对所有后端通用）
pub fn build_extraction_prompt(slices: &[MarkSlice]) -> String {
    let mut prompt = String::from(
        "你是终端工作流分析专家。\n\
         以下是终端录制内容，按 MOLT_MARK 分段，每段对应用户打下的一个语义锚点。\n\n\
         请分析这些终端操作，提取成可复用的 Pipeline YAML。\n\n",
    );

    for slice in slices {
        if slice.mark_index == 0 {
            prompt.push_str("--- 录制开始 ---\n");
        } else {
            prompt.push_str(&format!("--- MARK {} ---\n", slice.mark_index));
            if let Some(label) = &slice.label {
                prompt.push_str(&format!("标签: {}\n", label));
            }
        }
        prompt.push_str(&slice.content);
        prompt.push_str("\n\n");
    }

    prompt.push_str(
        "请严格按以下格式返回，只返回 YAML 代码块，不要任何其他说明：\n\n\
         ```yaml\n\
         name: my-pipeline\n\
         description: 简短描述\n\
         created_at: \"2026-04-12T00:00:00Z\"\n\
         steps:\n\
           - name: 步骤名称\n\
             mark: 1\n\
             cmd: \"实际执行的命令\"\n\
             description: 这步做什么\n\
             executor: local\n\
         ```\n\n\
         executor: local（本机）/ feishu_bot（委托 ClawBot）/ ask（需人工确认）\n\
         cmd 填写录制中实际出现的命令，不填 TODO 或注释。",
    );

    prompt
}
