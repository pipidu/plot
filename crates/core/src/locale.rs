//! 界面语言。默认简体中文。绘图和界面在显示时读取这里，不写进项目文件。

use std::cell::Cell;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[serde(rename = "zh")]
    #[default]
    Zh,
    #[serde(rename = "en")]
    En,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::Zh => "简体中文",
            Language::En => "English",
        }
    }

    pub fn all() -> [Language; 2] {
        [Language::Zh, Language::En]
    }
}

thread_local! {
    static CURRENT: Cell<Language> = const { Cell::new(Language::Zh) };
}

pub fn language() -> Language {
    CURRENT.with(|cell| cell.get())
}

pub fn set_language(language: Language) {
    CURRENT.with(|cell| cell.set(language));
}

pub fn translate<'a>(zh: &'a str, en: &'a str) -> &'a str {
    match language() {
        Language::Zh => zh,
        Language::En => en,
    }
}

/// 按当前语言取界面句子。中文是原文；英文查对照表，没有译文时仍显示中文。
pub fn t(zh: &str) -> &str {
    match language() {
        Language::Zh => zh,
        Language::En => crate::catalog::en(zh).unwrap_or(zh),
    }
}

#[macro_export]
macro_rules! tr {
    ($zh:literal, $en:literal) => {
        $crate::translate($zh, $en)
    };
}

#[macro_export]
macro_rules! trf {
    ($zh:literal, $en:literal) => {{
        match $crate::language() {
            $crate::Language::Zh => format!($zh),
            $crate::Language::En => format!($en),
        }
    }};
    ($zh:literal, $en:literal, $($rest:tt)*) => {{
        match $crate::language() {
            $crate::Language::Zh => format!($zh, $($rest)*),
            $crate::Language::En => format!($en, $($rest)*),
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_defaults_to_chinese_and_can_switch() {
        let _reset = ResetLanguage;
        assert_eq!(language(), Language::Zh);
        assert_eq!(t("取消"), "取消");
        set_language(Language::En);
        assert_eq!(t("取消"), "Cancel");
        assert_eq!(t("没有对照的句子"), "没有对照的句子");
        assert_eq!(crate::graph::CurveKind::Line.label(), "Line");
        set_language(Language::Zh);
        assert_eq!(t("取消"), "取消");
        assert_eq!(crate::graph::CurveKind::Line.label(), "折线");
    }

    #[test]
    fn english_demo_uses_english_names() {
        let _reset = ResetLanguage;
        set_language(Language::En);
        let project = crate::demo_project();
        assert_eq!(project.tables[0].name, "Trigonometry");
        assert!(project.notes[0].text.contains("Import CSV"));
        assert!(project.graphs.iter().any(|graph| graph.name == "Sine and cosine"));
    }

    struct ResetLanguage;
    impl Drop for ResetLanguage {
        fn drop(&mut self) {
            set_language(Language::Zh);
        }
    }
}
