use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde_json::{Map, Number, Value};
use yaml_rust2::{Yaml, YamlLoader};

use crate::domain::{
    document::MarkdownDocument,
    projection::{FrontmatterStatus, MarkdownProjection, ProjectionLink, ProjectionLinkKind},
};

/// UTF-8 Markdown snapshot을 검색 index용 projection으로 변환합니다.
///
/// `Parser`는 filesystem이나 `SQLite`에 접근하지 않으며 malformed frontmatter도 오류로
/// 반환하지 않습니다. 따라서 검색 metadata 품질 저하가 파일 CRUD를 차단하지 않습니다.
#[derive(Clone, Copy, Debug, Default)]
pub struct MarkdownProjectionParser;

impl MarkdownProjectionParser {
    #[must_use]
    pub fn parse(document: &MarkdownDocument) -> MarkdownProjection {
        let parsed_frontmatter = parse_frontmatter(&document.content);
        let parse_body = parsed_frontmatter
            .as_ref()
            .map_or(document.content.as_str(), |frontmatter| frontmatter.body);
        let metadata = parsed_frontmatter
            .as_ref()
            .and_then(|frontmatter| parse_yaml_metadata(frontmatter.source));
        let frontmatter_status = match (&parsed_frontmatter, &metadata) {
            (None, _) if starts_with_frontmatter_delimiter(&document.content) => {
                FrontmatterStatus::Malformed
            }
            (None, _) => FrontmatterStatus::Absent,
            (Some(_), Some(_)) => FrontmatterStatus::Parsed,
            (Some(_), None) => FrontmatterStatus::Malformed,
        };
        let body = if frontmatter_status == FrontmatterStatus::Parsed {
            parse_body
        } else {
            document.content.as_str()
        };
        let markdown_metadata = parse_markdown(body);
        let mut tags = metadata
            .as_ref()
            .map_or_else(Vec::new, |metadata| metadata.tags.clone());
        append_unique(&mut tags, markdown_metadata.tags);
        let title = metadata
            .as_ref()
            .and_then(|metadata| metadata.title.clone())
            .or(markdown_metadata.first_h1)
            .unwrap_or_else(|| filename_title(document.path.as_str()));

        MarkdownProjection {
            path: document.path.clone(),
            title,
            body: body.to_owned(),
            tags,
            links: markdown_metadata.links,
            content_hash: document.hash.clone(),
            frontmatter_json: metadata.map(|metadata| metadata.json),
            frontmatter_status,
        }
    }
}

#[derive(Debug)]
struct Frontmatter<'a> {
    source: &'a str,
    body: &'a str,
}

#[derive(Debug)]
struct FrontmatterMetadata {
    title: Option<String>,
    tags: Vec<String>,
    json: String,
}

#[derive(Debug, Default)]
struct MarkdownMetadata {
    first_h1: Option<String>,
    tags: Vec<String>,
    links: Vec<ProjectionLink>,
}

fn parse_frontmatter(content: &str) -> Option<Frontmatter<'_>> {
    let delimiter_length = if content.starts_with("---\n") {
        4
    } else if content.starts_with("---\r\n") {
        5
    } else {
        return None;
    };
    let mut offset = delimiter_length;

    for segment in content[delimiter_length..].split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line == "---" || line == "..." {
            return Some(Frontmatter {
                source: &content[delimiter_length..offset],
                body: &content[offset + segment.len()..],
            });
        }
        offset += segment.len();
    }
    None
}

fn starts_with_frontmatter_delimiter(content: &str) -> bool {
    content == "---" || content.starts_with("---\n") || content.starts_with("---\r\n")
}

fn parse_yaml_metadata(source: &str) -> Option<FrontmatterMetadata> {
    if source.trim().is_empty() {
        return Some(FrontmatterMetadata {
            title: None,
            tags: Vec::new(),
            json: "{}".to_owned(),
        });
    }

    let mut documents = YamlLoader::load_from_str(source).ok()?;
    if documents.len() != 1 {
        return None;
    }
    let root = documents.pop()?;
    let mapping = root.as_hash()?;
    let json = serde_json::to_string(&yaml_to_json(&root)?).ok()?;
    let title = yaml_string(mapping.get(&Yaml::String("title".to_owned())))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let tags = extract_frontmatter_tags(mapping.get(&Yaml::String("tags".to_owned())));

    Some(FrontmatterMetadata { title, tags, json })
}

fn yaml_to_json(yaml: &Yaml) -> Option<Value> {
    match yaml {
        Yaml::Real(value) => value
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map_or_else(
                || Some(Value::String(value.clone())),
                |value| Some(Value::Number(value)),
            ),
        Yaml::Integer(value) => Some(Value::Number(Number::from(*value))),
        Yaml::String(value) => Some(Value::String(value.clone())),
        Yaml::Boolean(value) => Some(Value::Bool(*value)),
        Yaml::Array(values) => values
            .iter()
            .map(yaml_to_json)
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        Yaml::Hash(values) => {
            let mut mapping = Map::new();
            for (key, value) in values {
                let Yaml::String(key) = key else {
                    return None;
                };
                mapping.insert(key.clone(), yaml_to_json(value)?);
            }
            Some(Value::Object(mapping))
        }
        Yaml::Null => Some(Value::Null),
        Yaml::Alias(_) | Yaml::BadValue => None,
    }
}

fn yaml_string(value: Option<&Yaml>) -> Option<&str> {
    value.and_then(Yaml::as_str)
}

fn extract_frontmatter_tags(value: Option<&Yaml>) -> Vec<String> {
    let values = match value {
        Some(Yaml::String(value)) => value.split(',').collect(),
        Some(Yaml::Array(values)) => values.iter().filter_map(Yaml::as_str).collect(),
        _ => Vec::new(),
    };
    let mut tags = Vec::new();
    for value in values {
        push_tag(&mut tags, value);
    }
    tags
}

fn parse_markdown(body: &str) -> MarkdownMetadata {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut metadata = MarkdownMetadata::default();
    let mut heading = None;
    let mut in_code_block = false;
    let mut in_link_or_image = false;
    let mut code_block_start = None;
    let mut excluded_ranges = Vec::new();

    for (event, source_range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) if metadata.first_h1.is_none() => heading = Some(String::new()),
            Event::End(TagEnd::Heading(HeadingLevel::H1)) => {
                if let Some(value) = heading.take() {
                    let value = value.trim();
                    if !value.is_empty() {
                        metadata.first_h1 = Some(value.to_owned());
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                code_block_start = Some(source_range.start);
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                if let Some(start) = code_block_start.take() {
                    excluded_ranges.push(start..source_range.end);
                }
            }
            Event::Start(Tag::Link { dest_url, .. }) if !in_code_block => {
                in_link_or_image = true;
                push_link(&mut metadata.links, ProjectionLinkKind::Markdown, &dest_url);
            }
            Event::Start(Tag::Image { dest_url, .. }) if !in_code_block => {
                in_link_or_image = true;
                push_link(
                    &mut metadata.links,
                    ProjectionLinkKind::MarkdownImage,
                    &dest_url,
                );
            }
            Event::End(TagEnd::Link | TagEnd::Image) => in_link_or_image = false,
            Event::Text(value) if !in_code_block => {
                if let Some(heading) = heading.as_mut() {
                    heading.push_str(&value);
                }
                if !in_link_or_image {
                    extract_inline_tags(&value, &mut metadata.tags);
                }
            }
            Event::Code(value) => {
                excluded_ranges.push(source_range);
                if let Some(heading) = heading.as_mut() {
                    heading.push_str(&value);
                }
            }
            Event::Html(_) | Event::InlineHtml(_) => excluded_ranges.push(source_range),
            Event::SoftBreak | Event::HardBreak if heading.is_some() => {
                heading.as_mut().expect("heading is present").push(' ');
            }
            _ => {}
        }
    }
    extract_source_metadata(body, &excluded_ranges, &mut metadata);
    metadata
}

fn extract_source_metadata(
    body: &str,
    excluded_ranges: &[Range<usize>],
    metadata: &mut MarkdownMetadata,
) {
    let mut cursor = 0;
    for range in excluded_ranges {
        if cursor < range.start {
            extract_wiki_links(&body[cursor..range.start], &mut metadata.links);
        }
        cursor = cursor.max(range.end);
    }
    if cursor < body.len() {
        extract_wiki_links(&body[cursor..], &mut metadata.links);
    }
}

fn extract_inline_tags(text: &str, tags: &mut Vec<String>) {
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    for (index, &(byte_index, character)) in characters.iter().enumerate() {
        if character != '#' || !has_tag_boundary(text, byte_index) {
            continue;
        }
        let start = byte_index + character.len_utf8();
        let end = characters[index + 1..]
            .iter()
            .find_map(|(position, value)| (!is_tag_character(*value)).then_some(*position))
            .unwrap_or(text.len());
        if start < end {
            push_tag(tags, &text[start..end]);
        }
    }
}

fn has_tag_boundary(text: &str, byte_index: usize) -> bool {
    text[..byte_index]
        .chars()
        .next_back()
        .is_none_or(|character| !character.is_alphanumeric() && character != '_')
}

fn is_tag_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '/')
}

fn push_tag(tags: &mut Vec<String>, value: &str) {
    let normalized = value
        .trim()
        .trim_start_matches('#')
        .trim_end_matches('/')
        .to_lowercase();
    if !normalized.is_empty() && !tags.contains(&normalized) {
        tags.push(normalized);
    }
}

fn extract_wiki_links(text: &str, links: &mut Vec<ProjectionLink>) {
    let mut remainder = text;
    while let Some(opening) = remainder.find("[[") {
        let embedded = remainder[..opening].ends_with('!');
        let candidate = &remainder[opening + 2..];
        let Some(closing) = candidate.find("]]") else {
            break;
        };
        let target = candidate[..closing]
            .split_once('|')
            .map_or(&candidate[..closing], |(target, _)| target);
        push_link(
            links,
            if embedded {
                ProjectionLinkKind::WikiEmbed
            } else {
                ProjectionLinkKind::Wiki
            },
            target,
        );
        remainder = &candidate[closing + 2..];
    }
}

fn push_link(links: &mut Vec<ProjectionLink>, kind: ProjectionLinkKind, target: &str) {
    let target = target.trim();
    if target.is_empty() {
        return;
    }
    let link = ProjectionLink {
        kind,
        target: target.to_owned(),
    };
    if !links.contains(&link) {
        links.push(link);
    }
}

fn append_unique(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

fn filename_title(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|filename| filename.strip_suffix(".md"))
        .unwrap_or(path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::MarkdownProjectionParser;
    use crate::domain::{
        document::MarkdownDocument,
        path::MarkdownPath,
        projection::{FrontmatterStatus, ProjectionLink, ProjectionLinkKind},
    };

    fn document(path: &str, content: &str) -> MarkdownDocument {
        MarkdownDocument {
            path: MarkdownPath::parse(path).expect("test Markdown path must be valid"),
            content: content.to_owned(),
            hash: format!("sha256:{}", "a".repeat(64)),
            size: content.len() as u64,
            modified_at: SystemTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn extracts_frontmatter_projection_and_removes_it_from_body() {
        let source =
            "---\ntitle: 검색 설계\ntags:\n  - Search\n  - 한글\nstatus: draft\n---\n\n본문";
        let projection = MarkdownProjectionParser::parse(&document("notes/search.md", source));

        assert_eq!(projection.title, "검색 설계");
        assert_eq!(projection.body, "\n본문");
        assert_eq!(projection.tags, ["search", "한글"]);
        assert_eq!(projection.frontmatter_status, FrontmatterStatus::Parsed);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                projection
                    .frontmatter_json
                    .as_deref()
                    .expect("frontmatter JSON should exist")
            )
            .expect("frontmatter JSON should be valid"),
            serde_json::json!({
                "title": "검색 설계",
                "tags": ["Search", "한글"],
                "status": "draft"
            })
        );
        assert_eq!(
            projection.content_hash,
            format!("sha256:{}", "a".repeat(64))
        );
    }

    #[test]
    fn falls_back_from_frontmatter_title_to_first_h1_then_filename() {
        let heading = MarkdownProjectionParser::parse(&document(
            "notes/fallback.md",
            "## 먼저\n# **실제** `제목`\n본문",
        ));
        let filename =
            MarkdownProjectionParser::parse(&document("notes/파일 제목.md", "제목 없는 본문"));

        assert_eq!(heading.title, "실제 제목");
        assert_eq!(filename.title, "파일 제목");
    }

    #[test]
    fn malformed_frontmatter_keeps_complete_raw_body_and_fallback_metadata() {
        let source = "---\ntitle: [broken\n---\n# 안전한 제목\n본문 #fallback";
        let projection = MarkdownProjectionParser::parse(&document("broken.md", source));

        assert_eq!(projection.frontmatter_status, FrontmatterStatus::Malformed);
        assert_eq!(projection.frontmatter_json, None);
        assert_eq!(projection.body, source);
        assert_eq!(projection.title, "안전한 제목");
        assert_eq!(projection.tags, ["fallback"]);
    }

    #[test]
    fn unclosed_frontmatter_is_malformed_without_losing_content() {
        let source = "---\ntitle: 끝나지 않음\n# 본문";
        let projection = MarkdownProjectionParser::parse(&document("unclosed.md", source));

        assert_eq!(projection.frontmatter_status, FrontmatterStatus::Malformed);
        assert_eq!(projection.body, source);
    }

    #[test]
    fn accepts_empty_and_crlf_frontmatter() {
        let empty = MarkdownProjectionParser::parse(&document("empty.md", "---\n---\n# Empty"));
        let crlf = MarkdownProjectionParser::parse(&document(
            "crlf.md",
            "---\r\ntitle: CRLF\r\n...\r\n# ignored",
        ));

        assert_eq!(empty.frontmatter_status, FrontmatterStatus::Parsed);
        assert_eq!(empty.frontmatter_json.as_deref(), Some("{}"));
        assert_eq!(crlf.title, "CRLF");
        assert_eq!(crlf.body, "# ignored");
    }

    #[test]
    fn merges_normalized_frontmatter_and_inline_tags_in_source_order() {
        let source = "---\ntags: Search, KnowledgeOS\n---\n#search #한글/검색 #KnowledgeOS";
        let projection = MarkdownProjectionParser::parse(&document("tags.md", source));

        assert_eq!(projection.tags, ["search", "knowledgeos", "한글/검색"]);
    }

    #[test]
    fn ignores_hashtags_inside_inline_and_fenced_code() {
        let source = "본문 #real `#inline` <https://example.com/#fragment>\n```text\n#fenced\n```";
        let projection = MarkdownProjectionParser::parse(&document("tags.md", source));

        assert_eq!(projection.tags, ["real"]);
    }

    #[test]
    fn extracts_standard_wiki_embed_and_image_links() {
        let source = concat!(
            "[API](./api.md) ![diagram](images/design.png)\n",
            "[[architecture#section|Architecture]] ![[assets/map.png]]"
        );
        let projection = MarkdownProjectionParser::parse(&document("links.md", source));

        assert_eq!(
            projection.links,
            [
                ProjectionLink {
                    kind: ProjectionLinkKind::Markdown,
                    target: "./api.md".to_owned(),
                },
                ProjectionLink {
                    kind: ProjectionLinkKind::MarkdownImage,
                    target: "images/design.png".to_owned(),
                },
                ProjectionLink {
                    kind: ProjectionLinkKind::Wiki,
                    target: "architecture#section".to_owned(),
                },
                ProjectionLink {
                    kind: ProjectionLinkKind::WikiEmbed,
                    target: "assets/map.png".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn deduplicates_links_and_ignores_code_block_references() {
        let source = concat!(
            "[one](same.md) [two](same.md) [[wiki]] [[wiki]]\n",
            "`[[inline-hidden]]`\n```\n[[fenced-hidden]]\n```\n",
            "<!-- [[html-hidden]] -->"
        );
        let projection = MarkdownProjectionParser::parse(&document("links.md", source));

        assert_eq!(projection.links.len(), 2);
        assert!(
            !projection
                .links
                .iter()
                .any(|link| link.target.contains("hidden"))
        );
    }

    #[test]
    fn rejects_non_mapping_or_non_json_compatible_frontmatter_as_metadata_only() {
        for source in [
            "---\n- item\n---\n본문",
            "---\n? [complex, key]\n: value\n---\n본문",
        ] {
            let projection = MarkdownProjectionParser::parse(&document("invalid.md", source));

            assert_eq!(projection.frontmatter_status, FrontmatterStatus::Malformed);
            assert_eq!(projection.body, source);
            assert_eq!(projection.frontmatter_json, None);
        }
    }
}
