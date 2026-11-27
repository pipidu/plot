//! 界面语言。默认简体中文。绘图和界面在显示时读取这里，不写进项目文件。

use std::cell::Cell;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    #[serde(rename = "zh")]
    #[default]
    Zh,
    /// 繁体中文。
    #[serde(rename = "zh-Hant")]
    ZhHant,
    #[serde(rename = "en")]
    En,
    /// 德语。
    #[serde(rename = "de")]
    De,
    /// 法语。
    #[serde(rename = "fr")]
    Fr,
    /// 俄语。
    #[serde(rename = "ru")]
    Ru,
    /// 西班牙语。
    #[serde(rename = "es")]
    Es,
    /// 捷克语。
    #[serde(rename = "cs")]
    Cs,
    /// 巴西葡萄牙语。
    #[serde(rename = "pt-BR")]
    PtBr,
}

impl Language {
    pub fn label(self) -> &'static str {
        match self {
            Language::Zh => "简体中文",
            Language::ZhHant => "繁體中文",
            Language::En => "English",
            Language::De => "Deutsch",
            Language::Fr => "Français",
            Language::Ru => "Русский",
            Language::Es => "Español",
            Language::Cs => "Čeština",
            Language::PtBr => "Português (Brasil)",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Language::Zh => "zh",
            Language::ZhHant => "zh-Hant",
            Language::En => "en",
            Language::De => "de",
            Language::Fr => "fr",
            Language::Ru => "ru",
            Language::Es => "es",
            Language::Cs => "cs",
            Language::PtBr => "pt-BR",
        }
    }

    pub fn all() -> &'static [Language] {
        &[
            Language::Zh,
            Language::ZhHant,
            Language::En,
            Language::De,
            Language::Fr,
            Language::Ru,
            Language::Es,
            Language::Cs,
            Language::PtBr,
        ]
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
        other => crate::lang_extra::phrase(other, zh).unwrap_or(en),
    }
}

/// 按当前语言取界面句子。中文是原文。英文查对照表。
/// 其他语言先查自己的词表，没有就用英文，再没有就留中文。
pub fn t(zh: &str) -> &str {
    match language() {
        Language::Zh => zh,
        Language::En => crate::catalog::en(zh).unwrap_or(zh),
        other => crate::lang_extra::phrase(other, zh)
            .or_else(|| crate::catalog::en(zh))
            .unwrap_or(zh),
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
            _ => format!($en),
        }
    }};
    ($zh:literal, $en:literal, $($rest:tt)*) => {{
        match $crate::language() {
            $crate::Language::Zh => format!($zh, $($rest)*),
            _ => format!($en, $($rest)*),
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
    fn more_languages_translate_the_shell_and_fall_back() {
        let _reset = ResetLanguage;
        assert_eq!(Language::all().len(), 9);
        let expect = [
            (Language::ZhHant, "檔案"),
            (Language::De, "Datei"),
            (Language::Fr, "Fichier"),
            (Language::Ru, "Файл"),
            (Language::Es, "Archivo"),
            (Language::Cs, "Soubor"),
            (Language::PtBr, "Arquivo"),
        ];
        for (language, file) in expect {
            set_language(language);
            assert!(!language.label().is_empty());
            assert_eq!(t("文件"), file, "{}", language.code());
            assert_eq!(t("脚本"), match language {
                Language::ZhHant => "腳本",
                Language::De => "Skript",
                Language::Fr => "Script",
                Language::Ru => "Скрипт",
                Language::Es => "Script",
                Language::Cs => "Skript",
                Language::PtBr => "Script",
                _ => "",
            });
            assert_eq!(t("取消"), match language {
                Language::ZhHant => "取消",
                Language::De => "Abbrechen",
                Language::Fr => "Annuler",
                Language::Ru => "Отмена",
                Language::Es => "Cancelar",
                Language::Cs => "Zrušit",
                Language::PtBr => "Cancelar",
                _ => "",
            });
            // 词表没有这条，英文对照有，就用英文。
            assert_eq!(t("乙"), "B");
            // 两边都没有，留简体中文。
            assert_eq!(t("没有对照的句子"), "没有对照的句子");
            let text = serde_json::to_string(&language).unwrap();
            let back: Language = serde_json::from_str(&text).unwrap();
            assert_eq!(back, language);
        }
        set_language(Language::En);
        assert_eq!(t("文件"), "File");
        assert_eq!(t("乙"), "B");
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
