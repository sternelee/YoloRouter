// 15 维度请求分析和模型评分系统
// 支持多语言检测、任务类型识别和智能场景匹配

use crate::models::ChatMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Scenario metadata tuple: (name, match_task_types, match_languages, priority, is_default)
pub type ScenarioMeta<'a> = (&'a str, &'a [String], &'a [String], i32, bool);

// ─── Public Types ─────────────────────────────────────────────────────────────

/// Detected language of the request
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Language {
    /// Chinese / Japanese / Korean
    Cjk,
    /// Latin-script languages (English, French, German, etc.)
    Latin,
    /// Primarily source code
    Code,
    /// Mixed content
    Mixed,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Cjk => "cjk",
            Language::Latin => "latin",
            Language::Code => "code",
            Language::Mixed => "mixed",
        }
    }
}

/// Detected task type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TaskType {
    Coding,
    Reasoning,
    General,
    Creative,
    Translation,
    Analysis,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Coding => "coding",
            TaskType::Reasoning => "reasoning",
            TaskType::General => "general",
            TaskType::Creative => "creative",
            TaskType::Translation => "translation",
            TaskType::Analysis => "analysis",
        }
    }
}

/// Extracted features from the actual request content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestFeatures {
    pub language: Language,
    pub task_type: TaskType,
    pub estimated_tokens: usize,
    pub confidence: f32,
    pub requires_vision: bool,
    pub requires_tools: bool,
}

/// 15 维度分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestAnalysis {
    pub complexity_score: f32,
    pub cost_importance: f32,
    pub latency_requirement: f32,
    pub accuracy_requirement: f32,
    pub throughput_requirement: f32,
    pub cost_budget_remaining: f32,
    pub availability_score: f32,
    pub cache_hit_score: f32,
    pub geo_compliance_score: f32,
    pub privacy_level: f32,
    pub feature_requirement: f32,
    pub reliability_requirement: f32,
    pub reasoning_score: f32,
    pub coding_score: f32,
    pub general_knowledge_score: f32,
    pub features: RequestFeatures,
}

/// Score for one configured model candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScore {
    pub model_id: String,
    pub overall_score: f32,
    pub estimated_cost: f32,
    pub estimated_latency_ms: f32,
    pub meets_constraints: bool,
    pub reasoning: String,
}

/// Candidate model supplied by the routing engine
#[derive(Debug, Clone)]
pub struct ModelCandidate {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub capabilities: Vec<String>,
    pub cost_tier: String,
}

// ─── FastAnalyzer ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct ModelCost {
    pub input_price_per_1m_tokens: f32,
    pub output_price_per_1m_tokens: f32,
}

pub struct FastAnalyzer {
    model_perf: HashMap<String, [f32; 15]>,
    model_costs: HashMap<String, ModelCost>,
}

impl FastAnalyzer {
    pub fn new() -> Self {
        Self {
            model_perf: Self::default_perf_matrix(),
            model_costs: Self::default_cost_table(),
        }
    }

    /// Analyse messages and score candidates. Use only for /v1/auto routing.
    pub fn analyze_and_score(
        &self,
        messages: &[ChatMessage],
        candidates: &[ModelCandidate],
    ) -> (RequestAnalysis, Vec<ModelScore>) {
        let features = Self::extract_features(messages);
        let analysis = self.build_analysis(&features);
        let mut scores: Vec<ModelScore> = candidates
            .iter()
            .map(|c| self.score_candidate(c, &analysis))
            .collect();
        scores.sort_by(|a, b| b.overall_score.partial_cmp(&a.overall_score).unwrap());
        (analysis, scores)
    }

    /// Extract features from request messages without full scoring.
    pub fn extract_features(messages: &[ChatMessage]) -> RequestFeatures {
        let content: String = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let estimated_tokens = (content.len() / 4).max(1);
        let language = detect_language(&content);
        let (task_type, task_confidence) = detect_task_type(&content, &language);
        let lang_confidence = if matches!(language, Language::Mixed) {
            0.6
        } else {
            0.9
        };
        let confidence = (task_confidence * lang_confidence).clamp(0.0, 1.0);

        let requires_vision = messages
            .iter()
            .any(|m| m.content.contains("image:") || m.content.contains("data:image/"));
        let requires_tools = messages
            .iter()
            .any(|m| m.content.contains("tool_call") || m.content.contains("function_call"));

        RequestFeatures {
            language,
            task_type,
            estimated_tokens,
            confidence,
            requires_vision,
            requires_tools,
        }
    }

    fn build_analysis(&self, f: &RequestFeatures) -> RequestAnalysis {
        let complexity = Self::complexity_from_features(f);
        let (reasoning, coding, knowledge) = task_dimensions(&f.task_type);

        RequestAnalysis {
            complexity_score: complexity,
            cost_importance: 50.0,
            latency_requirement: 50.0,
            accuracy_requirement: accuracy_from_task(&f.task_type),
            throughput_requirement: 1.0,
            cost_budget_remaining: 1000.0,
            availability_score: 90.0,
            cache_hit_score: 20.0,
            geo_compliance_score: 100.0,
            privacy_level: 30.0,
            feature_requirement: feature_score(f.requires_vision, f.requires_tools),
            reliability_requirement: 90.0,
            reasoning_score: reasoning,
            coding_score: coding,
            general_knowledge_score: knowledge,
            features: f.clone(),
        }
    }

    fn score_candidate(&self, c: &ModelCandidate, a: &RequestAnalysis) -> ModelScore {
        if a.features.requires_vision && !c.capabilities.contains(&"vision".to_string()) {
            return ModelScore {
                model_id: c.id.clone(),
                overall_score: 0.0,
                estimated_cost: 0.0,
                estimated_latency_ms: 0.0,
                meets_constraints: false,
                reasoning: format!("{} lacks vision capability", c.model),
            };
        }

        let base_score = if let Some(perf) = self.model_perf.get(&c.id) {
            self.weighted_score(perf, a)
        } else {
            self.heuristic_score(c, a)
        };

        let estimated_cost = self
            .model_costs
            .get(&c.id)
            .map(|mc| {
                (a.features.estimated_tokens as f32 / 1_000_000.0) * mc.input_price_per_1m_tokens
                    + (500.0 / 1_000_000.0) * mc.output_price_per_1m_tokens
            })
            .unwrap_or_default();

        let estimated_latency_ms = match c.cost_tier.as_str() {
            "high" => 3000.0,
            "medium" => 1500.0,
            _ => 600.0,
        };

        ModelScore {
            model_id: c.id.clone(),
            overall_score: base_score.clamp(0.0, 100.0),
            estimated_cost,
            estimated_latency_ms,
            meets_constraints: true,
            reasoning: format!(
                "{} score={:.0} task={} lang={}",
                c.model,
                base_score,
                a.features.task_type.as_str(),
                a.features.language.as_str()
            ),
        }
    }

    fn weighted_score(&self, perf: &[f32; 15], a: &RequestAnalysis) -> f32 {
        let weights: [f32; 15] = [
            a.complexity_score / 100.0,
            a.cost_importance / 100.0,
            a.latency_requirement / 100.0,
            a.accuracy_requirement / 100.0,
            a.throughput_requirement / 100.0,
            0.3,
            0.8,
            0.4,
            0.6,
            a.privacy_level / 100.0,
            a.feature_requirement / 100.0,
            a.reliability_requirement / 100.0,
            a.reasoning_score / 100.0,
            a.coding_score / 100.0,
            a.general_knowledge_score / 100.0,
        ];
        let weighted_sum: f32 = perf.iter().zip(weights.iter()).map(|(p, w)| p * w).sum();
        let weight_total: f32 = weights.iter().sum();
        if weight_total > 0.0 {
            weighted_sum / weight_total
        } else {
            50.0
        }
    }

    fn heuristic_score(&self, c: &ModelCandidate, a: &RequestAnalysis) -> f32 {
        let base: f32 = match c.cost_tier.as_str() {
            "high" => 75.0,
            "medium" => 60.0,
            _ => 45.0,
        };
        let mut bonus = 0.0f32;
        if a.coding_score > 70.0 && c.capabilities.contains(&"code".to_string()) {
            bonus += 15.0;
        }
        if a.reasoning_score > 70.0 && c.capabilities.contains(&"reasoning".to_string()) {
            bonus += 10.0;
        }
        base + bonus
    }

    fn complexity_from_features(f: &RequestFeatures) -> f32 {
        let mut s = (f.estimated_tokens as f32 / 4000.0 * 100.0).min(100.0);
        if f.requires_vision {
            s = (s + 20.0).min(100.0);
        }
        if f.requires_tools {
            s = (s + 15.0).min(100.0);
        }
        s
    }

    fn default_perf_matrix() -> HashMap<String, [f32; 15]> {
        let mut m: HashMap<String, [f32; 15]> = HashMap::new();

        // ─── OpenAI GPT-5 Series (Latest Frontier Agentic Models) ───────────────────

        m.insert(
            "gpt-5.4".to_string(),
            [
                98., 5., 40., 99., 15., 5., 80., 60., 70., 75., 70., 80., 99., 99., 98.,
            ],
        );
        m.insert(
            "gpt-5.4-mini".to_string(),
            [
                95., 35., 65., 97., 50., 35., 85., 75., 80., 80., 80., 85., 96., 98., 95.,
            ],
        );
        m.insert(
            "gpt-5.3-codex".to_string(),
            [
                96., 25., 55., 98., 35., 25., 82., 70., 75., 78., 82., 82., 98., 99., 96.,
            ],
        );
        m.insert(
            "gpt-5.2".to_string(),
            [
                94., 30., 60., 96., 45., 30., 84., 72., 78., 80., 78., 83., 95., 97., 94.,
            ],
        );
        m.insert(
            "gpt-5.2-codex".to_string(),
            [
                93., 28., 58., 95., 42., 28., 83., 71., 76., 78., 80., 82., 94., 98., 93.,
            ],
        );
        m.insert(
            "gpt-5.1".to_string(),
            [
                92., 32., 62., 94., 48., 32., 83., 70., 76., 78., 76., 82., 93., 96., 92.,
            ],
        );
        m.insert(
            "gpt-5-mini".to_string(),
            [
                88., 70., 85., 90., 80., 70., 85., 75., 82., 78., 80., 85., 88., 92., 88.,
            ],
        );

        // ─── OpenAI GPT-4 Series (Stable, Proven) ──────────────────────────────────

        m.insert(
            "gpt-4o".to_string(),
            [
                90., 45., 80., 92., 60., 45., 90., 82., 85., 82., 92., 90., 90., 93., 90.,
            ],
        );
        m.insert(
            "gpt-4o-mini".to_string(),
            [
                72., 82., 92., 78., 88., 82., 88., 75., 80., 75., 80., 80., 75., 80., 82.,
            ],
        );
        m.insert(
            "gpt-4-turbo".to_string(),
            [
                92., 40., 75., 94., 55., 40., 92., 85., 87., 88., 92., 92., 92., 94., 92.,
            ],
        );
        m.insert(
            "gpt-4".to_string(),
            [
                88., 50., 85., 90., 65., 50., 88., 80., 85., 85., 90., 88., 88., 92., 88.,
            ],
        );

        // ─── OpenAI Reasoning Models (o-series) ─────────────────────────────────────

        m.insert(
            "o1".to_string(),
            [
                98., 10., 45., 99., 20., 10., 85., 65., 75., 80., 75., 85., 99., 92., 95.,
            ],
        );
        m.insert(
            "o1-preview".to_string(),
            [
                95., 15., 50., 98., 25., 15., 88., 70., 80., 85., 80., 90., 98., 88., 90.,
            ],
        );
        m.insert(
            "o1-mini".to_string(),
            [
                92., 70., 75., 95., 80., 70., 88., 78., 82., 82., 78., 88., 95., 85., 88.,
            ],
        );

        // ─── Legacy Models ──────────────────────────────────────────────────────────

        m.insert(
            "gpt-3.5-turbo".to_string(),
            [
                62., 90., 97., 68., 95., 90., 85., 72., 78., 70., 72., 72., 62., 72., 75.,
            ],
        );

        // ─── Anthropic Claude Series ────────────────────────────────────────────────

        m.insert(
            "claude-opus-4.6".to_string(),
            [
                96., 18., 68., 98., 38., 18., 93., 82., 87., 91., 92., 96., 96., 89., 96.,
            ],
        );
        m.insert(
            "claude-opus-4.5".to_string(),
            [
                95., 20., 70., 97., 40., 20., 92., 80., 85., 90., 90., 95., 95., 88., 95.,
            ],
        );
        m.insert(
            "claude-sonnet-4.6".to_string(),
            [
                92., 48., 80., 94., 62., 48., 92., 82., 87., 88., 88., 92., 92., 88., 92.,
            ],
        );
        m.insert(
            "claude-sonnet-4.5".to_string(),
            [
                85., 55., 82., 90., 65., 55., 90., 78., 85., 85., 85., 90., 88., 85., 88.,
            ],
        );
        m.insert(
            "claude-haiku-4.5".to_string(),
            [
                65., 85., 95., 75., 90., 85., 90., 72., 80., 80., 75., 82., 72., 75., 80.,
            ],
        );

        m
    }

    fn default_cost_table() -> HashMap<String, ModelCost> {
        let mut c: HashMap<String, ModelCost> = HashMap::new();

        // ─── OpenAI GPT-5 Series Pricing ───────────────────────────────────────────
        c.insert(
            "gpt-5.4".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 25.0,
                output_price_per_1m_tokens: 100.0,
            },
        );
        c.insert(
            "gpt-5.4-mini".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 5.0,
                output_price_per_1m_tokens: 20.0,
            },
        );
        c.insert(
            "gpt-5.3-codex".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 20.0,
                output_price_per_1m_tokens: 80.0,
            },
        );
        c.insert(
            "gpt-5.2".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 18.0,
                output_price_per_1m_tokens: 72.0,
            },
        );
        c.insert(
            "gpt-5.2-codex".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 18.0,
                output_price_per_1m_tokens: 72.0,
            },
        );
        c.insert(
            "gpt-5.1".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 15.0,
                output_price_per_1m_tokens: 60.0,
            },
        );
        c.insert(
            "gpt-5-mini".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 0.5,
                output_price_per_1m_tokens: 2.0,
            },
        );

        // ─── OpenAI GPT-4 Series Pricing ───────────────────────────────────────────
        c.insert(
            "gpt-4o".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 2.5,
                output_price_per_1m_tokens: 10.0,
            },
        );
        c.insert(
            "gpt-4o-mini".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 0.15,
                output_price_per_1m_tokens: 0.6,
            },
        );
        c.insert(
            "gpt-4-turbo".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 10.0,
                output_price_per_1m_tokens: 30.0,
            },
        );
        c.insert(
            "gpt-4".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 3.0,
                output_price_per_1m_tokens: 6.0,
            },
        );

        // ─── OpenAI Reasoning Models (o-series) ────────────────────────────────────
        c.insert(
            "o1".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 20.0,
                output_price_per_1m_tokens: 80.0,
            },
        );
        c.insert(
            "o1-preview".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 15.0,
                output_price_per_1m_tokens: 60.0,
            },
        );
        c.insert(
            "o1-mini".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 3.0,
                output_price_per_1m_tokens: 12.0,
            },
        );

        // ─── Legacy Models ─────────────────────────────────────────────────────────
        c.insert(
            "gpt-3.5-turbo".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 0.5,
                output_price_per_1m_tokens: 1.5,
            },
        );

        // ─── Anthropic Claude Pricing (USD per 1M tokens) ──────────────────────────
        c.insert(
            "claude-opus-4.6".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 15.0,
                output_price_per_1m_tokens: 60.0,
            },
        );
        c.insert(
            "claude-opus-4.5".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 3.0,
                output_price_per_1m_tokens: 15.0,
            },
        );
        c.insert(
            "claude-sonnet-4.6".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 3.0,
                output_price_per_1m_tokens: 15.0,
            },
        );
        c.insert(
            "claude-sonnet-4.5".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 1.0,
                output_price_per_1m_tokens: 5.0,
            },
        );
        c.insert(
            "claude-haiku-4.5".to_string(),
            ModelCost {
                input_price_per_1m_tokens: 0.25,
                output_price_per_1m_tokens: 1.25,
            },
        );

        c
    }
}

impl Default for FastAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Language Detection ───────────────────────────────────────────────────────

fn detect_language(text: &str) -> Language {
    let total_chars = text.chars().count();
    if total_chars == 0 {
        return Language::Latin;
    }

    let mut cjk = 0usize;
    let mut code_indicators = 0usize;
    let mut structural_chars = 0usize;

    for ch in text.chars() {
        let cp = ch as u32;
        if (0x4E00..=0x9FFF).contains(&cp)
            || (0x3040..=0x309F).contains(&cp)
            || (0x30A0..=0x30FF).contains(&cp)
            || (0xAC00..=0xD7AF).contains(&cp)
            || (0x3400..=0x4DBF).contains(&cp)
        {
            cjk += 1;
        }
        if matches!(ch, '(' | ')' | '{' | '}' | '[' | ']') {
            structural_chars += 1;
        }
    }

    let code_patterns = [
        "def ",
        "fn ",
        "func ",
        "function ",
        "class ",
        "struct ",
        "import ",
        "use ",
        "require(",
        "include ",
        "package ",
        "if (",
        "if(",
        "for (",
        "for(",
        "while(",
        "while (",
        "=>",
        "->",
        "::",
        "===",
        "!==",
        "&&",
        "||",
    ];
    for pattern in &code_patterns {
        if text.contains(pattern) {
            code_indicators += 1;
        }
    }

    let cjk_ratio = cjk as f32 / total_chars as f32;
    let structural_ratio = structural_chars as f32 / total_chars as f32;
    // High keyword density OR at least one keyword + structural punctuation
    let is_code = cjk_ratio < 0.15
        && (code_indicators >= 2 || (code_indicators >= 1 && structural_ratio > 0.04));

    if is_code {
        Language::Code
    } else if cjk_ratio > 0.25 {
        if code_indicators >= 2 {
            Language::Mixed
        } else {
            Language::Cjk
        }
    } else if cjk_ratio > 0.05 {
        Language::Mixed
    } else {
        Language::Latin
    }
}

// ─── Task Type Detection ──────────────────────────────────────────────────────

fn detect_task_type(text: &str, lang: &Language) -> (TaskType, f32) {
    let lower = text.to_lowercase();

    let coding_en = [
        "write code",
        "implement",
        "debug",
        "function",
        "algorithm",
        "refactor",
        "unit test",
        "class ",
        "variable",
        "compile",
        "syntax",
        "code review",
        "program",
        "script",
        "regex",
        "sql",
        "api endpoint",
        "dockerfile",
        "kubernetes",
        "git ",
        "bash ",
        "shell ",
    ];
    let coding_zh = [
        "写代码",
        "实现",
        "调试",
        "函数",
        "算法",
        "重构",
        "单元测试",
        "类",
        "变量",
        "编程",
        "脚本",
        "代码审查",
        "接口",
    ];
    let reasoning_en = [
        "why ",
        "analyze",
        "explain",
        "compare",
        "evaluate",
        "pros and cons",
        "trade-off",
        "because",
        "therefore",
        "step by step",
        "reason",
        "logic",
        "prove",
        "hypothesis",
        "conclusion",
    ];
    let reasoning_zh = [
        "为什么",
        "分析",
        "解释",
        "比较",
        "评估",
        "优缺点",
        "权衡",
        "因为",
        "所以",
        "一步一步",
        "推理",
        "证明",
    ];
    let translation_en = [
        "translate",
        "translation",
        "into english",
        "into chinese",
        "into french",
        "into japanese",
        "into korean",
    ];
    let translation_zh = ["翻译", "译成", "转换成", "中文翻译", "英文翻译"];
    let creative_en = [
        "write a story",
        "poem",
        "essay",
        "creative",
        "fiction",
        "narrative",
        "blog post",
        "article",
        "script",
    ];
    let creative_zh = ["写故事", "写诗", "创意", "小说", "散文", "文章"];
    let analysis_en = [
        "data analysis",
        "statistics",
        "chart",
        "graph",
        "dataset",
        "correlation",
        "regression",
        "visualize",
        "insights",
        "trends",
    ];
    let analysis_zh = ["数据分析", "统计", "图表", "数据集", "可视化", "趋势"];

    fn count_hits(text: &str, keywords: &[&str]) -> f32 {
        keywords.iter().filter(|kw| text.contains(*kw)).count() as f32
    }

    let is_code_lang = matches!(lang, Language::Code);
    let has_cjk = matches!(lang, Language::Cjk | Language::Mixed);

    let coding_score = count_hits(&lower, &coding_en)
        + if has_cjk {
            count_hits(&lower, &coding_zh) * 1.5
        } else {
            0.0
        }
        + if is_code_lang { 5.0 } else { 0.0 };

    let reasoning_score = count_hits(&lower, &reasoning_en)
        + if has_cjk {
            count_hits(&lower, &reasoning_zh) * 1.5
        } else {
            0.0
        };

    let translation_score = count_hits(&lower, &translation_en)
        + if has_cjk {
            count_hits(&lower, &translation_zh) * 1.5
        } else {
            0.0
        };

    let creative_score = count_hits(&lower, &creative_en)
        + if has_cjk {
            count_hits(&lower, &creative_zh) * 1.5
        } else {
            0.0
        };

    let analysis_score = count_hits(&lower, &analysis_en)
        + if has_cjk {
            count_hits(&lower, &analysis_zh) * 1.5
        } else {
            0.0
        };

    let scores = [
        ("coding", coding_score),
        ("reasoning", reasoning_score),
        ("translation", translation_score),
        ("creative", creative_score),
        ("analysis", analysis_score),
        ("general", 0.5_f32),
    ];

    let total: f32 = scores.iter().map(|(_, s)| s).sum::<f32>() + 1.0;
    let best = scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .copied()
        .unwrap_or(("general", 0.5));

    let confidence = if best.1 < 1.0 {
        0.45
    } else {
        (best.1 / total).clamp(0.0, 1.0)
    };

    let task_type = match best.0 {
        "coding" => TaskType::Coding,
        "reasoning" => TaskType::Reasoning,
        "translation" => TaskType::Translation,
        "creative" => TaskType::Creative,
        "analysis" => TaskType::Analysis,
        _ => TaskType::General,
    };

    (task_type, confidence)
}

// ─── Dimension helpers ────────────────────────────────────────────────────────

fn task_dimensions(task: &TaskType) -> (f32, f32, f32) {
    match task {
        TaskType::Coding => (55.0, 90.0, 65.0),
        TaskType::Reasoning => (90.0, 45.0, 75.0),
        TaskType::Analysis => (80.0, 55.0, 80.0),
        TaskType::Translation => (40.0, 20.0, 85.0),
        TaskType::Creative => (50.0, 15.0, 80.0),
        TaskType::General => (65.0, 35.0, 85.0),
    }
}

fn accuracy_from_task(task: &TaskType) -> f32 {
    match task {
        TaskType::Coding => 90.0,
        TaskType::Reasoning => 85.0,
        TaskType::Analysis => 85.0,
        _ => 70.0,
    }
}

fn feature_score(vision: bool, tools: bool) -> f32 {
    let mut s = 80.0f32;
    if vision {
        s -= 20.0;
    }
    if tools {
        s -= 15.0;
    }
    s.max(0.0)
}

// ─── Scenario matching ────────────────────────────────────────────────────────

/// Returns the best scenario name for the given analysis.
/// `scenario_metadata` entries: (name, match_task_types, match_languages, priority, is_default)
pub fn match_scenario(
    analysis: &RequestAnalysis,
    scenario_metadata: &[ScenarioMeta<'_>],
    confidence_threshold: f32,
) -> Option<String> {
    // If confidence is too low, skip content-based matching and use default
    if analysis.features.confidence < confidence_threshold {
        return scenario_metadata
            .iter()
            .find(|(.., is_default)| *is_default)
            .map(|(name, ..)| name.to_string());
    }

    let task_str = analysis.features.task_type.as_str();
    let lang_str = analysis.features.language.as_str();

    let mut best: Option<(&str, i32)> = None;
    for (name, task_types, languages, priority, _is_default) in scenario_metadata {
        let task_match = task_types.is_empty() || task_types.iter().any(|t| t == task_str);
        let lang_match =
            languages.is_empty() || languages.iter().any(|l| l == lang_str || l == "mixed");

        if task_match && lang_match && (best.is_none() || *priority > best.unwrap().1) {
            best = Some((name, *priority));
        }
    }

    best.map(|(name, _)| name.to_string()).or_else(|| {
        scenario_metadata
            .iter()
            .find(|(.., is_default)| *is_default)
            .map(|(name, ..)| name.to_string())
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(content: &str) -> ChatMessage {
        ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_language_detection_cjk() {
        let lang = detect_language("请帮我写一个Python函数来解析JSON数据");
        assert_eq!(lang, Language::Cjk);
    }

    #[test]
    fn test_language_detection_code() {
        let lang = detect_language("def parse_json(data: str):\n    return json.loads(data)");
        assert_eq!(lang, Language::Code);
    }

    #[test]
    fn test_language_detection_latin() {
        let lang = detect_language("What is the capital of France? Please explain.");
        assert_eq!(lang, Language::Latin);
    }

    #[test]
    fn test_task_detection_coding() {
        let (task, conf) = detect_task_type(
            "write code to implement a binary search algorithm",
            &Language::Latin,
        );
        assert_eq!(task, TaskType::Coding);
        assert!(
            conf > 0.4,
            "confidence {conf} should be reasonable for coding request"
        );
    }

    #[test]
    fn test_task_detection_reasoning() {
        let (task, _) = detect_task_type(
            "explain why quicksort is faster than bubble sort and analyze the trade-offs",
            &Language::Latin,
        );
        assert!(
            task == TaskType::Reasoning || task == TaskType::Analysis,
            "got {task:?}"
        );
    }

    #[test]
    fn test_task_detection_cjk_coding() {
        let (task, conf) = detect_task_type("请帮我写代码实现一个二分查找算法", &Language::Cjk);
        assert_eq!(task, TaskType::Coding);
        assert!(conf > 0.4, "confidence {conf}");
    }

    #[test]
    fn test_extract_features() {
        let messages = vec![msg(
            "Please implement a REST API endpoint in Rust using actix-web",
        )];
        let features = FastAnalyzer::extract_features(&messages);
        assert_eq!(features.task_type, TaskType::Coding);
        assert!(features.estimated_tokens > 5);
    }

    #[test]
    fn test_analyze_and_score_performance() {
        let analyzer = FastAnalyzer::new();
        let messages = vec![msg("Write a Python function to sort a list")];
        let candidates = vec![
            ModelCandidate {
                id: "anthropic/claude-opus-4-5".to_string(),
                provider: "anthropic".to_string(),
                model: "claude-opus-4-5".to_string(),
                capabilities: vec!["code".to_string()],
                cost_tier: "high".to_string(),
            },
            ModelCandidate {
                id: "openai/gpt-4o-mini".to_string(),
                provider: "openai".to_string(),
                model: "gpt-4o-mini".to_string(),
                capabilities: vec!["code".to_string()],
                cost_tier: "low".to_string(),
            },
        ];
        let start = std::time::Instant::now();
        let (_analysis, scores) = analyzer.analyze_and_score(&messages, &candidates);
        let elapsed_us = start.elapsed().as_micros();
        assert!(!scores.is_empty());
        assert!(
            elapsed_us < 5000,
            "analyze_and_score took {elapsed_us}us, expected <5ms"
        );
    }

    #[test]
    fn test_scenario_matching() {
        let analysis = RequestAnalysis {
            complexity_score: 60.0,
            cost_importance: 50.0,
            latency_requirement: 50.0,
            accuracy_requirement: 90.0,
            throughput_requirement: 1.0,
            cost_budget_remaining: 1000.0,
            availability_score: 90.0,
            cache_hit_score: 20.0,
            geo_compliance_score: 100.0,
            privacy_level: 30.0,
            feature_requirement: 80.0,
            reliability_requirement: 90.0,
            reasoning_score: 55.0,
            coding_score: 90.0,
            general_knowledge_score: 65.0,
            features: RequestFeatures {
                language: Language::Latin,
                task_type: TaskType::Coding,
                estimated_tokens: 200,
                confidence: 0.8,
                requires_vision: false,
                requires_tools: false,
            },
        };

        let coding_types: Vec<String> = vec!["coding".to_string(), "code_review".to_string()];
        let general_types: Vec<String> = vec![];
        let all_langs: Vec<String> = vec![];

        let metadata: Vec<(&str, &[String], &[String], i32, bool)> = vec![
            ("coding", &coding_types, &all_langs, 100, false),
            ("general", &general_types, &all_langs, 50, true),
        ];

        let matched = match_scenario(&analysis, &metadata, 0.6);
        assert_eq!(matched, Some("coding".to_string()));
    }

    #[test]
    fn test_scenario_matching_falls_back_to_default() {
        let analysis = RequestAnalysis {
            complexity_score: 50.0,
            cost_importance: 50.0,
            latency_requirement: 50.0,
            accuracy_requirement: 70.0,
            throughput_requirement: 1.0,
            cost_budget_remaining: 1000.0,
            availability_score: 90.0,
            cache_hit_score: 20.0,
            geo_compliance_score: 100.0,
            privacy_level: 30.0,
            feature_requirement: 80.0,
            reliability_requirement: 90.0,
            reasoning_score: 50.0,
            coding_score: 40.0,
            general_knowledge_score: 70.0,
            features: RequestFeatures {
                language: Language::Latin,
                task_type: TaskType::General,
                estimated_tokens: 50,
                confidence: 0.3, // low confidence
                requires_vision: false,
                requires_tools: false,
            },
        };

        let coding_types: Vec<String> = vec!["coding".to_string()];
        let general_types: Vec<String> = vec![];
        let all_langs: Vec<String> = vec![];

        let metadata: Vec<(&str, &[String], &[String], i32, bool)> = vec![
            ("coding", &coding_types, &all_langs, 100, false),
            ("general", &general_types, &all_langs, 50, true),
        ];

        // Low confidence → should fall back to default scenario "general"
        let matched = match_scenario(&analysis, &metadata, 0.6);
        assert_eq!(matched, Some("general".to_string()));
    }
}
