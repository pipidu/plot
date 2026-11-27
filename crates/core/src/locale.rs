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
        $crate::fill_trf($zh, format!($zh), format!($en))
    }};
    ($zh:literal, $en:literal, $($rest:tt)*) => {{
        $crate::fill_trf($zh, format!($zh, $($rest)*), format!($en, $($rest)*))
    }};
}

/// 把带参数的句子填进当前语言。简体用中文，英语用英文。
/// 其余七种语言用 `lang_formats` 里的格式串，孔按中文的顺序填入已经格式化的文字。
/// 状态栏那一批都在词表里。没有收录的句子仍用英文，和第五批的短句回落一样。
pub fn fill_trf(zh_template: &str, zh_filled: String, en_filled: String) -> String {
    match language() {
        Language::Zh => zh_filled,
        Language::En => en_filled,
        other => {
            let Some(translated) = crate::lang_formats::template(other, zh_template) else {
                return en_filled;
            };
            apply_holes(translated, &extract_holes(zh_template, &zh_filled).unwrap_or_default())
                .unwrap_or(en_filled)
        }
    }
}

enum Part<'a> {
    Lit(&'a str),
    Hole,
}

fn split_template(template: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        if start > 0 {
            parts.push(Part::Lit(&rest[..start]));
        }
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            parts.push(Part::Lit(rest));
            return parts;
        };
        parts.push(Part::Hole);
        rest = &after[end + 1..];
    }
    if !rest.is_empty() {
        parts.push(Part::Lit(rest));
    }
    parts
}

fn extract_holes(template: &str, filled: &str) -> Option<Vec<String>> {
    let parts = split_template(template);
    let mut rest = filled;
    let mut values = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        match part {
            Part::Lit(lit) => {
                rest = rest.strip_prefix(lit)?;
            }
            Part::Hole => {
                let next = parts[index + 1..].iter().find_map(|part| match part {
                    Part::Lit(lit) if !lit.is_empty() => Some(*lit),
                    _ => None,
                });
                match next {
                    Some(lit) => {
                        let pos = rest.find(lit)?;
                        values.push(rest[..pos].to_string());
                        rest = &rest[pos..];
                    }
                    None => {
                        values.push(rest.to_string());
                        rest = "";
                    }
                }
            }
        }
    }
    if rest.is_empty() { Some(values) } else { None }
}

fn apply_holes(template: &str, values: &[String]) -> Option<String> {
    let parts = split_template(template);
    let holes = parts.iter().filter(|part| matches!(part, Part::Hole)).count();
    if holes != values.len() {
        return None;
    }
    let mut out = String::new();
    let mut index = 0;
    for part in parts {
        match part {
            Part::Lit(lit) => out.push_str(lit),
            Part::Hole => {
                out.push_str(&values[index]);
                index += 1;
            }
        }
    }
    Some(out)
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

    #[test]
    fn parameterized_status_uses_the_seven_languages() {
        let _reset = ResetLanguage;
        let expect = [
            (Language::ZhHant, "已儲存", "已列印"),
            (Language::De, "Gespeichert", "gedruckt"),
            (Language::Fr, "Enregistré", "imprimés"),
            (Language::Ru, "Сохранено", "Напечатано"),
            (Language::Es, "Guardado", "imprimieron"),
            (Language::Cs, "Uloženo", "Vytištěno"),
            (Language::PtBr, "Salvo", "impressos"),
        ];
        for (language, saved_word, print_word) in expect {
            set_language(language);
            let saved = crate::trf!("已保存 {}", "Saved {}", "demo.splot");
            assert!(saved.contains("demo.splot"), "{saved}");
            assert!(saved.contains(saved_word), "{saved}");
            assert!(!saved.contains("Saved"), "{saved}");
            let printed = crate::trf!(
                "已打印 {n} 张图到 {path}（PDF，{w:.0}×{h:.0} 点）。本机没有打印机时就写这个文件。",
                "Printed {n} graphs to {path} (PDF, {w:.0}×{h:.0} pt). Without a printer, this file is the printout.",
                n = 2,
                path = "out.pdf",
                w = 100.0,
                h = 200.0
            );
            assert!(printed.contains("out.pdf"), "{printed}");
            assert!(printed.contains(print_word), "{printed}");
            assert!(printed.contains("100"), "{printed}");
            assert!(printed.contains("200"), "{printed}");
            assert!(!printed.contains("Printed"), "{printed}");
            let missing = crate::trf!("完全没有收录 {}", "Totally missing {}", "x");
            assert_eq!(missing, "Totally missing x");
            let wrote = crate::trf!(
                "已写入 {written} 行，有 {missed} 行没有得到数值。{detail}",
                "Wrote {written} rows. {missed} rows did not produce a number. {detail}",
                written = 3,
                missed = 1,
                detail = "x"
            );
            assert!(wrote.contains('3') && wrote.contains('1') && wrote.contains('x'), "{wrote}");
            assert!(!wrote.contains("Wrote"), "{wrote}");
            let exported = crate::trf!("已导出 {}", "Exported {}", "out.svg");
            assert!(exported.contains("out.svg"), "{exported}");
            assert!(!exported.contains("Exported"), "{exported}");
        }
        set_language(Language::En);
        assert_eq!(crate::trf!("已保存 {}", "Saved {}", "a"), "Saved a");
        set_language(Language::Zh);
        assert_eq!(crate::trf!("已保存 {}", "Saved {}", "a"), "已保存 a");
    }

    struct ResetLanguage;
    impl Drop for ResetLanguage {
        fn drop(&mut self) {
            set_language(Language::Zh);
        }
    }
}
