use crate::artifact_hash::sha256_hex;
use crate::commands::health::issue;
use crate::constants::DEFAULT_DIR;
use crate::models::{Card, Manifest};
use crate::pack_io::{read_card, read_manifest, resolve_pack_path};
use crate::pack_readme::{
    README_INVENTORY_CONTRACT, extract_inventory_block, extract_ownership_block, fence_delimiter,
    human_owned_readme, markdown_line_offsets, render_inventory_block, render_ownership_block,
    replace_readme_regions, validate_readme_regions,
};
use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use serde_yaml::Value as YamlValue;
use std::fs;
use std::path::Path;

/// Compare the owned README inventory block against freshly loaded structured
/// authority. A legacy README without the owned marker is `unassessed` and never
/// blocks pack readiness; a stale owned block is `stale` and blocks.
pub(crate) fn check_readme(root: &Path) -> Result<Value> {
    let readme_path = root.join(DEFAULT_DIR).join("README.md");
    let existing = fs::read_to_string(&readme_path).unwrap_or_default();
    validate_readme_regions(&existing).map_err(|message| anyhow!(message))?;
    let existing_ownership = extract_ownership_block(&existing);
    let existing_inventory = extract_inventory_block(&existing);
    if existing_ownership.is_none() && existing_inventory.is_none() {
        let changed_generated_regions = vec!["ownership", "inventory"];
        return Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "unassessed",
            "valid": true,
            "drift": false,
            "has_owned_block": false,
            "has_ownership_region": false,
            "has_inventory_region": false,
            "path": readme_path.display().to_string(),
            "generated_regions": generated_region_report(&changed_generated_regions),
            "changed_generated_regions": changed_generated_regions,
            "untouched_human_regions": untouched_human_regions(),
            "semantic_prose_review": "not-performed",
            "warnings": []
        }));
    }
    let (manifest, cards, source_ledger, prompt_ids) = load_readme_authority(root)?;
    let warnings = human_reference_warnings(&existing, &manifest, &source_ledger, &readme_path);
    let card_refs = cards.iter().collect::<Vec<_>>();
    let fresh_block = render_inventory_block(&manifest, &card_refs, &source_ledger, &prompt_ids);
    let fresh_ownership = render_ownership_block();
    let changed_generated_regions = changed_generated_regions(&existing, Some(&fresh_block));

    if changed_generated_regions.is_empty() {
        Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "fresh",
            "valid": true,
            "drift": false,
            "has_owned_block": true,
            "has_ownership_region": true,
            "has_inventory_region": true,
            "path": readme_path.display().to_string(),
            "inventory_sha256": sha256_hex(fresh_block.as_bytes()),
            "generated_regions": generated_region_report(&changed_generated_regions),
            "changed_generated_regions": changed_generated_regions,
            "untouched_human_regions": untouched_human_regions(),
            "semantic_prose_review": "not-performed",
            "warnings": warnings
        }))
    } else {
        Ok(json!({
            "contract": README_INVENTORY_CONTRACT,
            "status": "stale",
            "valid": false,
            "drift": true,
            "has_owned_block": existing_inventory.is_some(),
            "has_ownership_region": existing_ownership.is_some(),
            "has_inventory_region": existing_inventory.is_some(),
            "path": readme_path.display().to_string(),
            "expected_sha256": sha256_hex(fresh_block.as_bytes()),
            "actual_sha256": existing_inventory.as_deref().map(|block| sha256_hex(block.as_bytes())),
            "generated_region_sha256": {
                "ownership": region_hash_evidence(&fresh_ownership, existing_ownership.as_deref()),
                "inventory": region_hash_evidence(&fresh_block, existing_inventory.as_deref())
            },
            "diagnostics": [stale_drift_issue(&readme_path, "error")],
            "generated_regions": generated_region_report(&changed_generated_regions),
            "changed_generated_regions": changed_generated_regions,
            "untouched_human_regions": untouched_human_regions(),
            "semantic_prose_review": "not-performed",
            "warnings": warnings
        }))
    }
}

fn region_hash_evidence(expected: &str, actual: Option<&str>) -> Value {
    json!({
        "expected": sha256_hex(expected.as_bytes()),
        "actual": actual.map(|region| sha256_hex(region.as_bytes()))
    })
}

/// Regenerate only the owned README inventory block, preserving arbitrary human
/// orientation prose outside the marker-delimited region. When the README has no
/// owned block yet, the block is appended as an explicit legacy migration.
pub(crate) fn refresh_readme(root: &Path, out: Option<&Path>, dry_run: bool) -> Result<Value> {
    let (manifest, cards, source_ledger, prompt_ids) = load_readme_authority(root)?;
    let card_refs = cards.iter().collect::<Vec<_>>();
    let fresh_block = render_inventory_block(&manifest, &card_refs, &source_ledger, &prompt_ids);
    let readme_path = root.join(DEFAULT_DIR).join("README.md");
    let existing = fs::read_to_string(&readme_path).unwrap_or_default();
    let updated =
        replace_readme_regions(&existing, &fresh_block).map_err(|message| anyhow!(message))?;
    let had_owned_block = extract_inventory_block(&existing).is_some();
    let changed_generated_regions = changed_generated_regions(&existing, Some(&fresh_block));
    let warnings = human_reference_warnings(&existing, &manifest, &source_ledger, &readme_path);
    let bytes = updated.len() as u64;

    let (status, target_path) = if dry_run {
        (
            "dry-run",
            out.map(|p| p.to_path_buf())
                .unwrap_or_else(|| readme_path.clone()),
        )
    } else {
        let write_target = out.unwrap_or(&readme_path);
        if let Some(parent) = write_target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::write(write_target, &updated)
            .with_context(|| format!("writing {}", write_target.display()))?;
        ("saved", write_target.to_path_buf())
    };

    let mut data = json!({
        "contract": README_INVENTORY_CONTRACT,
        "status": status,
        "path": readme_path.display().to_string(),
        "write_path": target_path.display().to_string(),
        "had_owned_block": had_owned_block,
        "inventory_sha256": sha256_hex(fresh_block.as_bytes()),
        "bytes": bytes,
        "block": fresh_block,
        "generated_regions": generated_region_report(&changed_generated_regions),
        "changed_generated_regions": changed_generated_regions,
        "untouched_human_regions": untouched_human_regions(),
        "semantic_prose_review": "not-performed",
        "warnings": warnings
    });
    if dry_run {
        if let Some(object) = data.as_object_mut() {
            object.insert("dry_run".to_string(), json!(true));
        }
    }
    Ok(data)
}

/// Validate-pack integration hook. Malformed generated-region markers are a
/// stable blocking validation error. Inventory drift remains warning-first;
/// missing README, legacy prose without markers, unrelated authority-read
/// failures, and a fresh block add no README-owned blocker.
pub(crate) fn readme_validation_issues(root: &Path) -> Vec<Value> {
    let readme_path = root.join(DEFAULT_DIR).join("README.md");
    if let Ok(existing) = fs::read_to_string(&readme_path)
        && let Err(message) = validate_readme_regions(&existing)
    {
        return vec![issue(
            "readme_marker_layout_invalid",
            "error",
            readme_path.display().to_string(),
            message,
        )];
    }
    let Some(result) = check_readme(root).ok() else {
        return vec![];
    };
    let mut issues = result["warnings"].as_array().cloned().unwrap_or_default();
    if result["status"] == "stale" {
        // Validate integration surfaces drift as a warning so legacy and
        // card-mutating flows remain compatible; strict validation promotes
        // it to a blocker. The standalone `readme check` command keeps its own
        // error-level diagnostic.
        issues.push(stale_drift_issue(
            &std::path::PathBuf::from(result["path"].as_str().unwrap_or(".mdp/README.md")),
            "warning",
        ));
    }
    issues
}

fn changed_generated_regions(existing: &str, fresh_inventory: Option<&str>) -> Vec<&'static str> {
    let mut changed = Vec::new();
    let fresh_ownership = render_ownership_block();
    if extract_ownership_block(existing).as_deref() != Some(fresh_ownership.as_str()) {
        changed.push("ownership");
    }
    if fresh_inventory
        .is_some_and(|fresh| extract_inventory_block(existing).as_deref() != Some(fresh))
    {
        changed.push("inventory");
    }
    changed
}

fn generated_region_report(changed: &[&str]) -> Value {
    json!([
        {"id": "ownership", "ownership": "machine", "changed": changed.contains(&"ownership")},
        {"id": "inventory", "ownership": "machine", "changed": changed.contains(&"inventory")}
    ])
}

fn untouched_human_regions() -> Value {
    json!([{
        "id": "readme-prose",
        "ownership": "human",
        "changed": false,
        "reviewed": false,
        "includes": ["thesis", "source interpretation", "gaps", "other prose"]
    }])
}

fn human_reference_warnings(
    readme: &str,
    manifest: &Manifest,
    source_ledger: &Value,
    readme_path: &Path,
) -> Vec<Value> {
    let human = human_owned_readme(readme);
    let card_paths = manifest
        .cards
        .iter()
        .filter_map(|card| normalize_card_path(&card.path))
        .collect::<std::collections::BTreeSet<_>>();
    let source_ids = source_ledger["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|source| source["id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut warnings = Vec::new();
    let mut seen_cards = std::collections::BTreeSet::new();
    for token in inline_code_tokens(&human) {
        let Some(normalized) = normalize_card_path(&token) else {
            continue;
        };
        if normalized.starts_with("cards/")
            && matches!(normalized.rsplit_once('.'), Some((_, "yaml" | "yml")))
            && !card_paths.contains(&normalized)
            && seen_cards.insert(normalized)
        {
            warnings.push(reference_warning(
                "readme_human_card_reference_missing",
                "card",
                &token,
                readme_path,
            ));
        }
    }
    let mut seen_sources = std::collections::BTreeSet::new();
    for token in source_reference_ids(&human) {
        if !source_ids.contains(token.as_str()) && seen_sources.insert(token.clone()) {
            warnings.push(reference_warning(
                "readme_human_source_reference_missing",
                "source",
                &token,
                readme_path,
            ));
        }
    }
    warnings
}

fn normalize_card_path(path: &str) -> Option<String> {
    let mut normalized = Vec::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(segment) => {
                normalized.push(segment.to_str()?.to_string());
            }
            // Card references are pack-relative. Never make absolute, parent,
            // root, or platform-prefix paths equivalent to an authority path.
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return None,
        }
    }
    (!normalized.is_empty()).then(|| normalized.join("/"))
}

fn reference_warning(code: &str, kind: &str, reference: &str, path: &Path) -> Value {
    let mut warning = issue(
        code,
        "warning",
        path.display().to_string(),
        format!(
            "Human-owned README prose references removed {kind} `{reference}`; refresh did not rewrite or semantically reconcile that prose."
        ),
    );
    warning["authority"] = json!("non-authoritative-mechanical-warning");
    warning["reference_kind"] = json!(kind);
    warning["reference"] = json!(reference);
    warning
}

fn inline_code_tokens(markdown: &str) -> Vec<String> {
    let mut fence: Option<(char, usize, MarkdownContainer)> = None;
    let mut indented_code = false;
    let mut indented_code_can_start = true;
    let mut container_state = MarkdownContainerState::default();
    let mut previous_container = None;
    let mut paragraph_open = false;
    let mut definition_title_pending: Option<MarkdownContainer> = None;
    let mut definition_destination_pending: Option<MarkdownContainer> = None;
    let mut open_definition_title: Option<(MarkdownContainer, char)> = None;
    let mut raw_html_end_tag = None;
    let mut raw_html_container: Option<MarkdownContainer> = None;
    let mut visible = String::with_capacity(markdown.len());
    for (start, content_end, line_end) in markdown_line_offsets(markdown) {
        let line = &markdown[start..content_end];
        let pending_definition_title = definition_title_pending.take();
        let pending_definition_destination = definition_destination_pending.take();
        let pending_open_definition_title = open_definition_title.take();
        let opening_container = fence.as_ref().map(|(_, _, container)| container);
        let (block_content, container, exited_container) =
            container_state.project(line, opening_container, paragraph_open);
        if exited_container {
            paragraph_open = false;
        }
        let definition_title_continuation =
            pending_definition_title
                .as_ref()
                .is_some_and(|opening_container| {
                    *opening_container == container
                        && project_container_path(line, opening_container).is_some()
                        && is_link_title_continuation(block_content)
                });
        let definition_destination_continuation = pending_definition_destination
            .as_ref()
            .filter(|opening_container| {
                **opening_container == container
                    && project_container_path(line, opening_container).is_some()
            })
            .and_then(|_| link_definition_continuation_title_state(block_content));
        if raw_html_end_tag.is_some() && raw_html_container.as_ref() != Some(&container) {
            raw_html_end_tag = None;
            raw_html_container = None;
        }
        if let Some((opening_container, closing)) = pending_open_definition_title
            && opening_container == container
            && project_container_path(line, &opening_container).is_some()
        {
            if let Some(closed) = multiline_title_line_state(block_content, closing) {
                if !closed {
                    open_definition_title = Some((opening_container, closing));
                }
                visible.push('\n');
                paragraph_open = false;
                continue;
            }
            paragraph_open = true;
        }
        let continuing_raw_html = raw_html_end_tag.is_some();
        if (raw_html_end_tag.is_some() || fence.is_none())
            && line_is_raw_html(block_content, &mut raw_html_end_tag, !paragraph_open)
        {
            if !continuing_raw_html && raw_html_end_tag.is_some() {
                raw_html_container = Some(container);
            } else if raw_html_end_tag.is_none() {
                raw_html_container = None;
            }
            visible.push('\n');
            paragraph_open = false;
            continue;
        }
        if previous_container.is_some_and(|previous| previous != container) {
            indented_code = false;
            indented_code_can_start = true;
        }
        previous_container = Some(container.clone());
        if fence
            .as_ref()
            .is_some_and(|(_, _, opening_container)| *opening_container != container)
        {
            // Block containers bound fenced code. Leaving an opening quote or
            // list item implicitly ends its unclosed fence; root prose must not
            // remain hidden until an unrelated matching delimiter appears.
            fence = None;
            indented_code_can_start = true;
        }
        let open_delimiter = fence
            .as_ref()
            .map(|(character, length, _)| (*character, *length));

        if let Some((character, length, closing)) = fence_delimiter(block_content, open_delimiter) {
            if closing {
                fence = None;
                indented_code_can_start = true;
            } else if fence.is_none() {
                fence = Some((character, length, container));
            }
            // Preserve a block boundary without exposing fence delimiters as
            // inline code or joining prose from opposite sides of the block.
            visible.push('\n');
            paragraph_open = false;
            continue;
        }
        if fence.is_some() {
            visible.push('\n');
            paragraph_open = false;
            continue;
        }

        let blank = block_content
            .bytes()
            .all(|byte| matches!(byte, b' ' | b'\t'));
        let indented = markdown_indent_columns(block_content) >= 4;
        if indented_code {
            if blank || indented {
                // Blank lines belong to an open indented block when a later
                // indented chunk continues it; either way they contain no
                // reference claim and preserve only a parsing boundary here.
                visible.push('\n');
                paragraph_open = false;
                continue;
            }
            indented_code = false;
        }
        if indented && indented_code_can_start {
            indented_code = true;
            visible.push('\n');
            paragraph_open = false;
            continue;
        }
        let definition_can_start = !paragraph_open;
        let definition_line = definition_can_start
            && (link_reference_title_state(block_content).is_some()
                || is_link_reference_destination_pending(block_content));
        if definition_line
            || definition_title_continuation
            || definition_destination_continuation.is_some()
        {
            visible.push('\n');
        } else {
            visible.push_str(&markdown[start..line_end]);
        }
        // CommonMark indented code cannot interrupt a paragraph. A blank line
        // re-enables block start; ordinary prose keeps later indentation in
        // the paragraph, where inline code spans remain mechanically visible.
        let pending_title_closer = ((definition_can_start
            && link_reference_title_state(block_content) == Some(false))
            || definition_destination_continuation == Some(false))
        .then(|| incomplete_link_title_closer(block_content))
        .flatten();
        open_definition_title = pending_title_closer.map(|closing| (container.clone(), closing));
        definition_title_pending = (pending_title_closer.is_none()
            && ((definition_can_start
                && link_reference_title_state(block_content) == Some(false))
                || definition_destination_continuation == Some(false)))
        .then_some(container.clone());
        definition_destination_pending = (definition_can_start
            && is_link_reference_destination_pending(block_content))
        .then_some(container.clone());
        let paragraph_continues = line_continues_paragraph(block_content, paragraph_open)
            || (paragraph_open
                && (markdown_indent_columns(block_content) >= 4
                    || is_link_reference_definition(block_content)));
        indented_code_can_start = blank
            || definition_title_continuation
            || definition_destination_continuation.is_some()
            || !paragraph_continues;
        paragraph_open = !definition_title_continuation
            && definition_destination_continuation.is_none()
            && paragraph_continues;
    }
    code_span_tokens(&visible)
}

fn markdown_indent_columns(line: &str) -> usize {
    let mut columns = 0;
    for byte in line.bytes() {
        match byte {
            b' ' => columns += 1,
            b'\t' => columns += 4 - (columns % 4),
            _ => break,
        }
    }
    columns
}

#[derive(Clone, Debug)]
enum RawHtmlEnd {
    Literal(&'static str),
    BlankLine,
}

fn line_is_raw_html(line: &str, end: &mut Option<RawHtmlEnd>, allow_complete_tag: bool) -> bool {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let lower = trimmed.to_ascii_lowercase();
    if let Some(termination) = end.as_ref() {
        let closed = match termination {
            RawHtmlEnd::Literal(literal) => lower.contains(literal),
            RawHtmlEnd::BlankLine => lower.trim().is_empty(),
        };
        if closed {
            *end = None;
        }
        return true;
    }
    if markdown_indent_columns(line) > 3 {
        return false;
    }
    for tag in ["script", "pre", "style", "textarea"] {
        let opener = format!("<{tag}");
        if lower.starts_with(&opener)
            && lower[opener.len()..]
                .chars()
                .next()
                .map_or(true, |character| {
                    character.is_ascii_whitespace() || character == '>'
                })
        {
            if !lower.contains(&format!("</{tag}>")) {
                *end = Some(RawHtmlEnd::Literal(match tag {
                    "script" => "</script>",
                    "pre" => "</pre>",
                    "style" => "</style>",
                    _ => "</textarea>",
                }));
            }
            return true;
        }
    }
    for (opener, closer) in [("<!--", "-->"), ("<?", "?>")] {
        if trimmed.starts_with(opener) {
            if !trimmed.contains(closer) {
                *end = Some(RawHtmlEnd::Literal(closer));
            }
            return true;
        }
    }
    if trimmed.starts_with("<![CDATA[") {
        if !trimmed.contains("]]>") {
            *end = Some(RawHtmlEnd::Literal("]]>"));
        }
        return true;
    }
    if trimmed.starts_with("<!")
        && trimmed
            .as_bytes()
            .get(2)
            .is_some_and(u8::is_ascii_uppercase)
    {
        if !trimmed.contains('>') {
            *end = Some(RawHtmlEnd::Literal(">"));
        }
        return true;
    }
    if starts_block_html_tag(&lower) {
        *end = Some(RawHtmlEnd::BlankLine);
        return true;
    }
    if allow_complete_tag && is_complete_html_tag(&lower) {
        *end = Some(RawHtmlEnd::BlankLine);
        return true;
    }
    false
}

fn is_complete_html_tag(line: &str) -> bool {
    let bytes = line.trim_end_matches([' ', '\t']).as_bytes();
    let mut index = 0;
    if bytes.get(index) != Some(&b'<') {
        return false;
    }
    index += 1;
    let closing = bytes.get(index) == Some(&b'/');
    index += usize::from(closing);
    if !bytes.get(index).is_some_and(u8::is_ascii_alphabetic) {
        return false;
    }
    index += 1;
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
    {
        index += 1;
    }
    if closing {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        return bytes.get(index) == Some(&b'>') && index + 1 == bytes.len();
    }
    loop {
        let separator_start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        let attribute_separated = index > separator_start;
        if bytes.get(index) == Some(&b'>') {
            return index + 1 == bytes.len();
        }
        if bytes.get(index) == Some(&b'/') && bytes.get(index + 1) == Some(&b'>') {
            return index + 2 == bytes.len();
        }
        if !attribute_separated {
            return false;
        }
        if !bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b':'))
        {
            return false;
        }
        index += 1;
        while bytes.get(index).is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
        }) {
            index += 1;
        }
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        match bytes.get(index).copied() {
            Some(quote @ (b'\'' | b'"')) => {
                index += 1;
                while bytes.get(index).is_some_and(|byte| *byte != quote) {
                    index += 1;
                }
                if bytes.get(index) != Some(&quote) {
                    return false;
                }
                index += 1;
            }
            Some(byte) if !byte.is_ascii_whitespace() && !b"\"'=<>`".contains(&byte) => {
                index += 1;
                while bytes
                    .get(index)
                    .is_some_and(|byte| !byte.is_ascii_whitespace() && !b"\"'=<>`".contains(byte))
                {
                    index += 1;
                }
            }
            _ => return false,
        }
    }
}

fn starts_block_html_tag(line: &str) -> bool {
    const TAGS: &[&str] = &[
        "address",
        "article",
        "aside",
        "base",
        "basefont",
        "blockquote",
        "body",
        "caption",
        "center",
        "col",
        "colgroup",
        "dd",
        "details",
        "dialog",
        "dir",
        "div",
        "dl",
        "dt",
        "fieldset",
        "figcaption",
        "figure",
        "footer",
        "form",
        "frame",
        "frameset",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "head",
        "header",
        "hr",
        "html",
        "iframe",
        "legend",
        "li",
        "link",
        "main",
        "menu",
        "menuitem",
        "nav",
        "noframes",
        "ol",
        "optgroup",
        "option",
        "p",
        "param",
        "search",
        "section",
        "summary",
        "table",
        "tbody",
        "td",
        "tfoot",
        "th",
        "thead",
        "title",
        "tr",
        "track",
        "ul",
    ];
    let candidate = line.strip_prefix("</").or_else(|| line.strip_prefix('<'));
    candidate.is_some_and(|candidate| {
        TAGS.iter().any(|tag| {
            candidate.strip_prefix(tag).is_some_and(|suffix| {
                suffix.is_empty()
                    || suffix.starts_with("/>")
                    || suffix.chars().next().is_some_and(|character| {
                        character.is_ascii_whitespace() || character == '>'
                    })
            })
        })
    })
}

fn line_continues_paragraph(line: &str, paragraph_open: bool) -> bool {
    let blank = line.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
    !blank
        && markdown_indent_columns(line) < 4
        && !is_atx_heading(line)
        && !is_thematic_or_setext_line(line, paragraph_open)
        && !is_link_reference_definition(line)
        && !line.trim_start_matches([' ', '\t']).starts_with("<!--")
}

fn line_interrupts_container_paragraph(line: &str) -> bool {
    let trimmed = line.trim_start_matches([' ', '\t']);
    if markdown_indent_columns(line) > 3 {
        return false;
    }
    let mut raw_html = None;
    trimmed.starts_with('>')
        || is_atx_heading(line)
        || list_item_content(line, false).is_some()
        || fence_delimiter(line, None).is_some()
        || is_thematic_or_setext_line(line, false)
        || line_is_raw_html(line, &mut raw_html, false)
}

fn is_atx_heading(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return false;
    }
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    (1..=6).contains(&hashes) && matches!(trimmed.as_bytes().get(hashes), None | Some(b' ' | b'\t'))
}

fn is_thematic_or_setext_line(line: &str, paragraph_open: bool) -> bool {
    let trimmed = line.trim_matches([' ', '\t']);
    if paragraph_open && !trimmed.is_empty() && trimmed.bytes().all(|byte| byte == b'=') {
        return true;
    }
    if paragraph_open && !trimmed.is_empty() && trimmed.bytes().all(|byte| byte == b'-') {
        return true;
    }
    for marker in [b'-', b'_', b'*'] {
        let count = trimmed
            .bytes()
            .filter(|byte| !matches!(byte, b' ' | b'\t'))
            .count();
        if count >= 3
            && trimmed
                .bytes()
                .all(|byte| byte == marker || matches!(byte, b' ' | b'\t'))
        {
            return true;
        }
    }
    false
}

fn is_link_reference_definition(line: &str) -> bool {
    link_reference_title_state(line).is_some()
}

fn link_reference_title_state(line: &str) -> Option<bool> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 || !trimmed.starts_with('[') {
        return None;
    }
    let mut escaped = false;
    let mut label_characters = 0usize;
    for (offset, character) in trimmed[1..].char_indices() {
        let index = 1 + offset;
        match character {
            '\\' if !escaped => escaped = true,
            ']' if !escaped => {
                return (index > 1
                    && trimmed[1..index]
                        .chars()
                        .any(|character| !character.is_whitespace())
                    && trimmed.as_bytes().get(index + 1) == Some(&b':'))
                .then(|| link_definition_suffix_title_state(&trimmed[index + 2..]))
                .flatten();
            }
            '[' if !escaped => return None,
            _ => escaped = false,
        }
        label_characters += 1;
        if label_characters > 999 {
            return None;
        }
    }
    None
}

fn is_link_reference_destination_pending(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 || !trimmed.starts_with('[') {
        return false;
    }
    let mut escaped = false;
    let mut label_characters = 0usize;
    for (offset, character) in trimmed[1..].char_indices() {
        let index = 1 + offset;
        match character {
            '\\' if !escaped => escaped = true,
            ']' if !escaped => {
                return index > 1
                    && trimmed[1..index]
                        .chars()
                        .any(|character| !character.is_whitespace())
                    && trimmed.as_bytes().get(index + 1) == Some(&b':')
                    && trimmed[index + 2..].trim_matches([' ', '\t']).is_empty();
            }
            '[' if !escaped => return false,
            _ => escaped = false,
        }
        label_characters += 1;
        if label_characters > 999 {
            return false;
        }
    }
    false
}

fn link_definition_suffix_title_state(suffix: &str) -> Option<bool> {
    let trailing = link_definition_trailing_title(suffix)?;
    if trailing.is_empty() {
        Some(false)
    } else if incomplete_title_closer(trailing).is_some() {
        Some(false)
    } else {
        valid_link_title(trailing).then_some(true)
    }
}

fn link_definition_trailing_title(suffix: &str) -> Option<&str> {
    let rest = suffix.trim_start_matches([' ', '\t']);
    if rest.is_empty() {
        return None;
    }
    let destination_end = if let Some(angle) = rest.strip_prefix('<') {
        let mut escaped = false;
        let mut closing = None;
        for (index, character) in angle.char_indices() {
            match character {
                '\\' if !escaped => escaped = true,
                '>' if !escaped => {
                    closing = Some(1 + index + character.len_utf8());
                    break;
                }
                '<' | '\n' | '\r' if !escaped => return None,
                character if character.is_whitespace() => return None,
                _ => escaped = false,
            }
        }
        let Some(closing) = closing else {
            return None;
        };
        closing
    } else {
        let mut depth = 0usize;
        let mut escaped = false;
        let mut end = 0usize;
        for (index, character) in rest.char_indices() {
            if character.is_whitespace() {
                break;
            }
            match character {
                '\\' if !escaped => escaped = true,
                '(' if !escaped => depth += 1,
                ')' if !escaped && depth > 0 => depth -= 1,
                ')' if !escaped => return None,
                character if character.is_control() && !escaped => return None,
                _ => escaped = false,
            }
            end = index + character.len_utf8();
        }
        if end == 0 || depth != 0 {
            return None;
        }
        end
    };
    Some(rest[destination_end..].trim_matches([' ', '\t']))
}

fn link_definition_continuation_title_state(line: &str) -> Option<bool> {
    (markdown_indent_columns(line) <= 3)
        .then(|| link_definition_suffix_title_state(line))
        .flatten()
}

fn valid_link_title(title: &str) -> bool {
    let Some(opening) = title.chars().next() else {
        return false;
    };
    let closing = match opening {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return false,
    };
    let mut escaped = false;
    for (index, character) in title[opening.len_utf8()..].char_indices() {
        match character {
            '\\' if !escaped => escaped = true,
            character if character == closing && !escaped => {
                return title[opening.len_utf8() + index + character.len_utf8()..]
                    .trim_matches([' ', '\t'])
                    .is_empty();
            }
            '\n' | '\r' if !escaped => return false,
            _ => escaped = false,
        }
    }
    false
}

fn incomplete_link_title_closer(line: &str) -> Option<char> {
    let suffix = reference_definition_suffix(line).unwrap_or(line);
    let trailing = link_definition_trailing_title(suffix)?;
    incomplete_title_closer(trailing)
}

fn reference_definition_suffix(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches(' ');
    if !trimmed.starts_with('[') {
        return None;
    }
    let mut escaped = false;
    let mut label_characters = 0usize;
    for (offset, character) in trimmed[1..].char_indices() {
        let index = 1 + offset;
        match character {
            '\\' if !escaped => escaped = true,
            ']' if !escaped => {
                return (trimmed.as_bytes().get(index + 1) == Some(&b':'))
                    .then_some(&trimmed[index + 2..]);
            }
            '[' if !escaped => return None,
            _ => escaped = false,
        }
        label_characters += 1;
        if label_characters > 999 {
            return None;
        }
    }
    None
}

fn incomplete_title_closer(title: &str) -> Option<char> {
    let opening = title.chars().next()?;
    if !matches!(opening, '"' | '\'' | '(') {
        return None;
    }
    let closing = if opening == '(' { ')' } else { opening };
    let mut escaped = false;
    for character in title[opening.len_utf8()..].chars() {
        match character {
            '\\' if !escaped => escaped = true,
            character if character == closing && !escaped => return None,
            _ => escaped = false,
        }
    }
    Some(closing)
}

fn multiline_title_line_state(line: &str, closing: char) -> Option<bool> {
    if markdown_indent_columns(line) > 3 || line.trim_matches([' ', '\t']).is_empty() {
        return None;
    }
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        match character {
            '\\' if !escaped => escaped = true,
            character if character == closing && !escaped => {
                return line[index + character.len_utf8()..]
                    .trim_matches([' ', '\t'])
                    .is_empty()
                    .then_some(true);
            }
            _ => escaped = false,
        }
    }
    Some(false)
}

fn is_link_title_continuation(line: &str) -> bool {
    let trimmed = line.trim_start_matches(' ');
    line.len() - trimmed.len() <= 3 && valid_link_title(trimmed)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MarkdownContainerSegment {
    Quote,
    List(usize),
}

type MarkdownContainer = Vec<MarkdownContainerSegment>;

#[derive(Default)]
struct MarkdownContainerState {
    active: MarkdownContainer,
    blank_lines: usize,
}

impl MarkdownContainerState {
    fn project<'a>(
        &mut self,
        line: &'a str,
        opening: Option<&MarkdownContainer>,
        paragraph_open: bool,
    ) -> (&'a str, MarkdownContainer, bool) {
        let blank = line.bytes().all(|byte| matches!(byte, b' ' | b'\t'));
        if let Some(opening) = opening {
            if let Some(content) = project_container_path(line, opening) {
                return (content, opening.clone(), false);
            }
            if blank
                && opening
                    .iter()
                    .any(|segment| matches!(segment, MarkdownContainerSegment::List(_)))
            {
                return ("", opening.clone(), false);
            }
        }

        if blank {
            self.blank_lines += 1;
            if self.blank_lines >= 2 {
                self.active.clear();
                self.blank_lines = 0;
            }
            return ("", self.active.clone(), false);
        }
        self.blank_lines = 0;

        let mut container = self.active.clone();
        let projection = (!container.is_empty())
            .then(|| project_container_path(line, &container))
            .flatten();
        let lazy_continuation = !container.is_empty()
            && projection.is_none()
            && paragraph_open
            && !line_interrupts_container_paragraph(line);
        let exited_container = !container.is_empty() && projection.is_none() && !lazy_continuation;
        let content = if container.is_empty() {
            line
        } else if let Some(content) = projection {
            content
        } else if lazy_continuation {
            line
        } else {
            container.clear();
            line
        };
        let content = parse_new_container_prefixes(
            content,
            &mut container,
            paragraph_open && !exited_container,
        );
        self.active = container.clone();
        (content, container, exited_container)
    }
}

fn project_container_path<'a>(
    mut line: &'a str,
    container: &[MarkdownContainerSegment],
) -> Option<&'a str> {
    for segment in container {
        line = match segment {
            MarkdownContainerSegment::Quote => strip_one_blockquote_prefix(line)?,
            MarkdownContainerSegment::List(indent) => strip_indent_columns(line, *indent)?,
        };
    }
    Some(line)
}

fn parse_new_container_prefixes<'a>(
    mut line: &'a str,
    container: &mut MarkdownContainer,
    mut paragraph_open: bool,
) -> &'a str {
    loop {
        if let Some(content) = strip_one_blockquote_prefix(line) {
            container.push(MarkdownContainerSegment::Quote);
            line = content;
            paragraph_open = false;
        } else if let Some((indent, content)) = list_item_content(line, paragraph_open) {
            container.push(MarkdownContainerSegment::List(indent));
            line = content;
            paragraph_open = false;
        } else {
            return line;
        }
    }
}

fn strip_one_blockquote_prefix(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let spaces = bytes
        .iter()
        .take(3)
        .take_while(|byte| **byte == b' ')
        .count();
    if bytes.get(spaces) != Some(&b'>') {
        return None;
    }
    let mut consumed = spaces + 1;
    if matches!(bytes.get(consumed), Some(b' ' | b'\t')) {
        consumed += 1;
    }
    Some(&line[consumed..])
}

fn list_item_content(line: &str, paragraph_open: bool) -> Option<(usize, &str)> {
    let bytes = line.as_bytes();
    let leading = bytes
        .iter()
        .take(3)
        .take_while(|byte| **byte == b' ')
        .count();
    let (marker_end, ordered_start) = match bytes.get(leading)? {
        b'-' | b'+' | b'*' => (leading + 1, None),
        byte if byte.is_ascii_digit() => {
            let mut cursor = leading;
            while bytes.get(cursor).is_some_and(u8::is_ascii_digit) && cursor - leading < 9 {
                cursor += 1;
            }
            if !matches!(bytes.get(cursor), Some(b'.' | b')')) {
                return None;
            }
            (cursor + 1, Some(&line[leading..cursor]))
        }
        _ => return None,
    };
    if paragraph_open && ordered_start.is_some_and(|start| start != "1") {
        return None;
    }
    let mut whitespace_end = marker_end;
    // Continuation indentation includes both the marker width and padding.
    // Counting leading whitespace alone makes `- ` require one column rather
    // than two and makes multi-digit ordered items even more permissive.
    let marker_column = marker_end;
    let mut content_column = marker_column;
    while let Some(byte @ (b' ' | b'\t')) = bytes.get(whitespace_end) {
        content_column = advance_markdown_column(content_column, *byte);
        whitespace_end += 1;
    }
    if whitespace_end == marker_end {
        return None;
    }
    if paragraph_open && whitespace_end == bytes.len() {
        return None;
    }
    let padding = content_column - marker_column;
    let (content_start, content_indent) = if padding <= 4 || whitespace_end == bytes.len() {
        (whitespace_end, content_column)
    } else {
        // With more than four columns after a marker, CommonMark uses one
        // whitespace character as list padding and leaves the excess as item
        // content. That remaining four-column indent can therefore start code.
        let first = bytes[marker_end];
        (
            marker_end + 1,
            advance_markdown_column(marker_column, first),
        )
    };
    Some((content_indent, &line[content_start..]))
}

fn advance_markdown_column(column: usize, byte: u8) -> usize {
    if byte == b'\t' {
        column + 4 - (column % 4)
    } else {
        column + 1
    }
}

fn strip_indent_columns(line: &str, required: usize) -> Option<&str> {
    let mut columns = 0;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' => columns += 1,
            b'\t' => columns += 4 - (columns % 4),
            _ => return (columns >= required).then_some(&line[index..]),
        }
        if columns >= required {
            return Some(&line[index + 1..]);
        }
    }
    (columns >= required).then_some("")
}

/// Project CommonMark-style backtick code spans in bounded linear time.
/// Delimiters close only on a later run of exactly the same length; line
/// endings inside a span normalize to spaces, and one surrounding ASCII space
/// is removed when both are present and the content is not all spaces.
fn code_span_tokens(markdown: &str) -> Vec<String> {
    let bytes = markdown.as_bytes();
    let mut runs = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'`' {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && bytes[cursor] == b'`' {
            cursor += 1;
        }
        runs.push((start, cursor, cursor - start));
    }

    let mut next_same = vec![None; runs.len()];
    let mut next_by_length = std::collections::BTreeMap::new();
    for (index, (_, _, length)) in runs.iter().enumerate().rev() {
        next_same[index] = next_by_length.insert(*length, index);
    }

    let mut tokens = Vec::new();
    let mut run_index = 0;
    while run_index < runs.len() {
        let Some(close_index) = next_same[run_index] else {
            run_index += 1;
            continue;
        };
        let (_, open_end, _) = runs[run_index];
        let (close_start, _, _) = runs[close_index];
        let normalized = normalize_code_span(&markdown[open_end..close_start]);
        if !normalized.is_empty() {
            tokens.push(normalized);
        }
        run_index = close_index + 1;
    }
    tokens
}

fn normalize_code_span(content: &str) -> String {
    let normalized = content.replace("\r\n", "\n").replace(['\r', '\n'], " ");
    if normalized.starts_with(' ')
        && normalized.ends_with(' ')
        && normalized.chars().any(|character| character != ' ')
    {
        normalized[1..normalized.len() - 1].to_string()
    } else {
        normalized
    }
}

fn source_reference_ids(markdown: &str) -> Vec<String> {
    let mut in_sources = false;
    let mut fence: Option<(char, usize, MarkdownContainer)> = None;
    let mut container_state = MarkdownContainerState::default();
    let mut paragraph_open = false;
    let mut definition_title_pending: Option<MarkdownContainer> = None;
    let mut definition_destination_pending: Option<MarkdownContainer> = None;
    let mut open_definition_title: Option<(MarkdownContainer, char)> = None;
    let mut raw_html_end_tag = None;
    let mut raw_html_container: Option<MarkdownContainer> = None;
    let mut ids = Vec::new();
    for (start, content_end, _) in markdown_line_offsets(markdown) {
        let line = &markdown[start..content_end];
        let pending_definition_title = definition_title_pending.take();
        let pending_definition_destination = definition_destination_pending.take();
        let pending_open_definition_title = open_definition_title.take();
        let opening_container = fence.as_ref().map(|(_, _, container)| container);
        let (block_content, container, exited_container) =
            container_state.project(line, opening_container, paragraph_open);
        if exited_container {
            paragraph_open = false;
        }
        let definition_title_continuation =
            pending_definition_title
                .as_ref()
                .is_some_and(|opening_container| {
                    *opening_container == container
                        && project_container_path(line, opening_container).is_some()
                        && is_link_title_continuation(block_content)
                });
        let definition_destination_continuation = pending_definition_destination
            .as_ref()
            .filter(|opening_container| {
                **opening_container == container
                    && project_container_path(line, opening_container).is_some()
            })
            .and_then(|_| link_definition_continuation_title_state(block_content));
        if raw_html_end_tag.is_some() && raw_html_container.as_ref() != Some(&container) {
            raw_html_end_tag = None;
            raw_html_container = None;
        }
        if let Some((opening_container, closing)) = pending_open_definition_title
            && opening_container == container
            && project_container_path(line, &opening_container).is_some()
        {
            if let Some(closed) = multiline_title_line_state(block_content, closing) {
                if !closed {
                    open_definition_title = Some((opening_container, closing));
                }
                paragraph_open = false;
                continue;
            }
            paragraph_open = true;
        }
        let continuing_raw_html = raw_html_end_tag.is_some();
        if (raw_html_end_tag.is_some() || fence.is_none())
            && line_is_raw_html(block_content, &mut raw_html_end_tag, !paragraph_open)
        {
            if !continuing_raw_html && raw_html_end_tag.is_some() {
                raw_html_container = Some(container);
            } else if raw_html_end_tag.is_none() {
                raw_html_container = None;
            }
            paragraph_open = false;
            continue;
        }
        if fence
            .as_ref()
            .is_some_and(|(_, _, opening_container)| *opening_container != container)
        {
            fence = None;
        }
        let open = fence
            .as_ref()
            .map(|(character, length, _)| (*character, *length));
        if let Some((character, length, closing)) = fence_delimiter(block_content, open) {
            if closing {
                fence = None;
            } else if fence.is_none() {
                fence = Some((character, length, container));
            }
            paragraph_open = false;
            continue;
        }
        if fence.is_some() {
            paragraph_open = false;
            continue;
        }
        if is_atx_heading(line) {
            let level = line
                .trim_start_matches(' ')
                .bytes()
                .take_while(|byte| *byte == b'#')
                .count();
            if level <= 2 {
                in_sources = line == "## Sources";
            }
            paragraph_open = false;
            continue;
        }
        let definition_can_start = !paragraph_open;
        let pending_title_closer = ((definition_can_start
            && link_reference_title_state(block_content) == Some(false))
            || definition_destination_continuation == Some(false))
        .then(|| incomplete_link_title_closer(block_content))
        .flatten();
        open_definition_title = pending_title_closer.map(|closing| (container.clone(), closing));
        definition_title_pending = (pending_title_closer.is_none()
            && ((definition_can_start
                && link_reference_title_state(block_content) == Some(false))
                || definition_destination_continuation == Some(false)))
        .then_some(container.clone());
        definition_destination_pending = (definition_can_start
            && is_link_reference_destination_pending(block_content))
        .then_some(container.clone());
        let paragraph_continues = line_continues_paragraph(block_content, paragraph_open)
            || (paragraph_open
                && (markdown_indent_columns(block_content) >= 4
                    || is_link_reference_definition(block_content)));
        paragraph_open = !definition_title_continuation
            && definition_destination_continuation.is_none()
            && paragraph_continues;
        if !in_sources {
            continue;
        }
        let Some(rest) = line.strip_prefix("- `") else {
            continue;
        };
        let Some((id, _)) = rest.split_once("`:") else {
            continue;
        };
        if !id.is_empty() {
            ids.push(id.to_string());
        }
    }
    ids
}

fn stale_drift_issue(readme_path: &Path, severity: &str) -> Value {
    issue(
        "readme_inventory_drift",
        severity,
        readme_path.display().to_string(),
        "A machine-owned README region does not match the canonical ownership legend or loaded structured authority; run `mdp readme refresh --dir .` to regenerate both owned regions, or remove unverifiable manual counts.",
    )
}

fn load_readme_authority(root: &Path) -> Result<(Manifest, Vec<Card>, Value, Vec<String>)> {
    let manifest = read_manifest(root)?;
    let mut cards = Vec::new();
    for card_ref in &manifest.cards {
        let path = resolve_pack_path(root, &card_ref.path)
            .with_context(|| format!("resolving card {}", card_ref.id))?;
        let card = read_card(&path)
            .with_context(|| format!("reading card {} for README inventory", card_ref.id))?;
        cards.push(card);
    }
    let source_ledger = load_source_ledger(root)?;
    let prompt_ids = load_prompt_ids(root)?;
    Ok((manifest, cards, source_ledger, prompt_ids))
}

fn load_source_ledger(root: &Path) -> Result<Value> {
    let path = root.join(DEFAULT_DIR).join("sources.yaml");
    if !path.exists() {
        return Ok(Value::Null);
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let yaml: YamlValue =
        serde_yaml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
    Ok(serde_json::to_value(&yaml).unwrap_or(Value::Null))
}

fn load_prompt_ids(root: &Path) -> Result<Vec<String>> {
    let prompts_dir = root.join(DEFAULT_DIR).join("prompts");
    if !prompts_dir.exists() {
        return Ok(vec![]);
    }
    let mut paths = fs::read_dir(&prompts_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    let mut ids = Vec::new();
    for path in paths {
        let extension = path.extension().and_then(|extension| extension.to_str());
        if !matches!(extension, Some("yaml" | "yml")) {
            continue;
        }
        let raw = match fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        let Ok(yaml) = serde_yaml::from_str::<YamlValue>(&raw) else {
            continue;
        };
        if let Some(id) = yaml["id"].as_str() {
            ids.push(id.to_string());
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::init::init_pack;
    use crate::models::{CardKind, Entry};
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn nonce() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    }

    #[test]
    fn fresh_generated_pack_has_fresh_inventory_block() {
        let root = std::env::temp_dir().join(format!("mdp-readme-fresh-{}", nonce()));
        init_pack(&root, "Fresh Readme Pack", "gtm", true, false).expect("pack should initialize");
        let result = check_readme(&root).expect("check should run");
        assert_eq!(result["status"], "fresh");
        assert_eq!(result["valid"], true);
        assert_eq!(result["drift"], false);
        assert_eq!(result["has_owned_block"], true);
        assert_eq!(result["changed_generated_regions"], json!([]));
        assert_eq!(result["semantic_prose_review"], "not-performed");
        assert!(
            std::fs::read_to_string(root.join(".mdp/README.md"))
                .unwrap()
                .contains(crate::pack_readme::README_OWNERSHIP_BEGIN)
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn modifying_a_card_after_rendering_makes_check_stale_with_drift_diagnostic() {
        let root = std::env::temp_dir().join(format!("mdp-readme-drift-{}", nonce()));
        init_pack(&root, "Drift Readme Pack", "gtm", true, false).expect("pack should initialize");
        // Mutate a card by appending an entry so the README inventory block is
        // now stale relative to loaded structured authority.
        let manifest = read_manifest(&root).expect("manifest");
        let pains_ref = manifest
            .cards
            .iter()
            .find(|card_ref| card_ref.kind == CardKind::Pains)
            .expect("pains card ref");
        let pains_path = resolve_pack_path(&root, &pains_ref.path).expect("path");
        let mut card = read_card(&pains_path).expect("card");
        card.entries.push(Entry {
            id: "extra-pain".into(),
            title: "Extra pain".into(),
            body: "added after rendering".into(),
            applies_to: vec![],
            scope: BTreeMap::new(),
            evidence: vec![],
            avoid: vec![],
            exact_paragraphs: None,
            constraints: Default::default(),
            metadata: BTreeMap::new(),
        });
        let serialized = serde_yaml::to_string(&card).expect("serialize");
        std::fs::write(&pains_path, serialized).expect("write card");

        let result = check_readme(&root).expect("check should run");
        assert_eq!(result["status"], "stale", "{result}");
        assert_eq!(result["valid"], false);
        assert_eq!(result["drift"], true);
        let diag = &result["diagnostics"][0];
        assert_eq!(diag["code"], "readme_inventory_drift");
        assert_eq!(diag["severity"], "error");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn editing_the_ownership_legend_makes_check_and_validate_stale() {
        let root = std::env::temp_dir().join(format!("mdp-readme-ownership-drift-{}", nonce()));
        init_pack(&root, "Ownership Drift Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("readme");
        let edited = readme.replacen(
            "Machine-owned: this ownership legend",
            "Machine-owned: edited ownership legend",
            1,
        );
        assert_ne!(edited, readme, "fixture must edit the owned legend");
        std::fs::write(&readme_path, edited).expect("write edited legend");

        let check = check_readme(&root).expect("check");
        assert_eq!(check["status"], "stale");
        assert_eq!(check["valid"], false);
        assert_eq!(check["changed_generated_regions"], json!(["ownership"]));
        assert_eq!(check["diagnostics"][0]["code"], "readme_inventory_drift");
        assert_ne!(
            check["generated_region_sha256"]["ownership"]["expected"],
            check["generated_region_sha256"]["ownership"]["actual"]
        );
        assert_eq!(
            check["generated_region_sha256"]["inventory"]["expected"],
            check["generated_region_sha256"]["inventory"]["actual"]
        );

        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        assert_eq!(validation["valid"], true, "drift stays warning-first");
        let drift = validation["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .find(|issue| issue["code"] == "readme_inventory_drift")
            .expect("ownership drift warning");
        assert_eq!(drift["severity"], "warning");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn partial_generated_region_presence_is_stale_in_both_directions() {
        let cases = [
            ("ownership", "inventory", json!(["inventory"])),
            ("inventory", "ownership", json!(["ownership"])),
        ];
        for (kept, removed, expected_changed) in cases {
            let root = std::env::temp_dir().join(format!("mdp-readme-partial-{kept}-{}", nonce()));
            init_pack(&root, "Partial Region Pack", "gtm", true, false)
                .expect("pack should initialize");
            let readme_path = root.join(".mdp/README.md");
            let readme = std::fs::read_to_string(&readme_path).expect("readme");
            let removed_block = if removed == "ownership" {
                extract_ownership_block(&readme).expect("ownership block")
            } else {
                extract_inventory_block(&readme).expect("inventory block")
            };
            let partial = readme.replacen(&removed_block, "", 1);
            std::fs::write(&readme_path, partial).expect("write partial readme");

            let check = check_readme(&root).expect("check");
            assert_eq!(check["status"], "stale", "kept {kept}");
            assert_eq!(check["valid"], false, "kept {kept}");
            assert_eq!(check["changed_generated_regions"], expected_changed);
            assert_eq!(check["has_ownership_region"], kept == "ownership");
            assert_eq!(check["has_inventory_region"], kept == "inventory");
            assert!(
                check["generated_region_sha256"][removed]["actual"].is_null(),
                "missing {removed} region must have null actual hash"
            );
            assert!(
                check["generated_region_sha256"][kept]["actual"].is_string(),
                "kept {kept} region must retain checksum evidence"
            );

            let validation = crate::commands::health::validate_pack(&root).expect("validate");
            assert_eq!(validation["valid"], true, "drift stays warning-first");
            assert!(validation["issues"].as_array().is_some_and(|issues| {
                issues.iter().any(|issue| {
                    issue["code"] == "readme_inventory_drift" && issue["severity"] == "warning"
                })
            }));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn refresh_regenerates_only_owned_block_and_is_byte_stable() {
        let root = std::env::temp_dir().join(format!("mdp-readme-refresh-{}", nonce()));
        init_pack(&root, "Refresh Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let before = std::fs::read_to_string(&readme_path).expect("readme");
        // Add human prose after the owned block and corrupt the block's counts.
        let corrupted = before.replace("- card entries:", "- card entries (hand-edited):");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted readme");
        assert!(check_readme(&root).expect("check")["status"] == "stale");

        let refreshed = refresh_readme(&root, None, false).expect("refresh");
        assert_eq!(refreshed["status"], "saved");
        assert_eq!(refreshed["changed_generated_regions"], json!(["inventory"]));
        assert_eq!(refreshed["untouched_human_regions"][0]["reviewed"], false);
        let after = std::fs::read_to_string(&readme_path).expect("readme after");
        assert!(after.contains("- card entries:"));
        assert!(!after.contains("hand-edited"));

        // Second refresh with unchanged authority is byte-stable.
        let again = refresh_readme(&root, None, false).expect("refresh again");
        let after_again = std::fs::read_to_string(&readme_path).expect("readme again");
        assert_eq!(
            after, after_again,
            "second refresh is byte-stable for unchanged authority"
        );
        assert_eq!(again["inventory_sha256"], refreshed["inventory_sha256"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_dry_run_does_not_write_and_reports_plan() {
        let root = std::env::temp_dir().join(format!("mdp-readme-dry-{}", nonce()));
        init_pack(&root, "Dry Run Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let before = std::fs::read_to_string(&readme_path).expect("readme");
        // Corrupt to confirm dry-run does not repair.
        let corrupted = before.replace("- cards:", "- packs:");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted readme");

        let out = root.join(".mdp").join("README.dry.md");
        let result = refresh_readme(&root, Some(&out), true).expect("dry run");
        assert_eq!(result["status"], "dry-run");
        assert_eq!(result["dry_run"], true);
        // The in-place README is unchanged by dry-run.
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("readme unchanged"),
            corrupted
        );
        // The requested out path was not written by dry-run.
        assert!(!out.exists(), "dry-run must not write the out path");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_changes_portable_hash_and_is_stable_for_unchanged_authority() {
        use crate::artifact_hash::pack_content_sha256;
        let root = std::env::temp_dir().join(format!("mdp-readme-hash-{}", nonce()));
        init_pack(&root, "Hash Readme Pack", "gtm", true, false).expect("pack should initialize");
        let hash_fresh = pack_content_sha256(&root).expect("hash");
        // Corrupt the owned block; the portable hash changes because README is
        // inside .mdp, even though structured authority is unchanged.
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("readme");
        let corrupted = readme.replace("- cards:", "- card count:");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted");
        let hash_corrupted = pack_content_sha256(&root).expect("hash");
        assert_ne!(hash_fresh, hash_corrupted);

        refresh_readme(&root, None, false).expect("refresh");
        let hash_repaired = pack_content_sha256(&root).expect("hash");
        // Repairing the owned block restores the byte-stable portable hash for
        // unchanged structured authority.
        assert_eq!(hash_fresh, hash_repaired);
        // A second refresh with unchanged authority keeps the hash stable.
        refresh_readme(&root, None, false).expect("refresh again");
        assert_eq!(hash_fresh, pack_content_sha256(&root).expect("hash"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_readme_without_marker_is_unassessed_and_does_not_block() {
        let root = std::env::temp_dir().join(format!("mdp-readme-legacy-{}", nonce()));
        init_pack(&root, "Legacy Readme Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        std::fs::write(&readme_path, "# Legacy Pack\n\nOrientation prose only.\n")
            .expect("write legacy readme");

        let result = check_readme(&root).expect("check should run");
        assert_eq!(result["status"], "unassessed");
        assert_eq!(result["valid"], true);
        assert_eq!(result["drift"], false);
        assert_eq!(result["has_owned_block"], false);
        assert_eq!(
            result["changed_generated_regions"],
            json!(["ownership", "inventory"])
        );
        assert_eq!(result["generated_regions"][0]["changed"], true);
        assert_eq!(result["generated_regions"][1]["changed"], true);
        let manifest = read_manifest(&root).expect("manifest");
        let missing_card = resolve_pack_path(&root, &manifest.cards[0].path).expect("card path");
        std::fs::remove_file(missing_card).expect("remove declared card");
        let broken_authority = check_readme(&root).expect("markerless check stays unassessed");
        assert_eq!(broken_authority["status"], "unassessed");
        assert_eq!(broken_authority["warnings"], json!([]));
        assert!(readme_validation_issues(&root).is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn quoted_inline_and_fenced_marker_text_stays_legacy_human_prose() {
        let root = std::env::temp_dir().join(format!("mdp-readme-quoted-markers-{}", nonce()));
        init_pack(&root, "Quoted Marker Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let human = format!(
            "Human orientation with inline `{}`.\n\n> {}\n\n```markdown\n{}\n{}\n{}\n{}\n```\n\nKeep these exact bytes.\n",
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            crate::pack_readme::README_INVENTORY_BEGIN,
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            crate::pack_readme::README_OWNERSHIP_END,
            crate::pack_readme::README_INVENTORY_BEGIN,
            crate::pack_readme::README_INVENTORY_END,
        );
        std::fs::write(&readme_path, &human).expect("write adversarial legacy README");

        let before = check_readme(&root).expect("check legacy README");
        assert_eq!(before["status"], "unassessed");
        assert_eq!(before["valid"], true);
        assert_eq!(
            before["changed_generated_regions"],
            json!(["ownership", "inventory"])
        );

        refresh_readme(&root, None, false).expect("refresh legacy README");
        let refreshed = std::fs::read_to_string(&readme_path).expect("refreshed README");
        assert!(
            refreshed.contains(&human),
            "refresh must preserve quoted, inline, and fenced marker prose byte-for-byte"
        );
        let after = check_readme(&root).expect("check refreshed README");
        assert_eq!(after["status"], "fresh");
        assert_eq!(after["changed_generated_regions"], json!([]));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_migrates_legacy_readme_by_appending_owned_block() {
        let root = std::env::temp_dir().join(format!("mdp-readme-migrate-{}", nonce()));
        init_pack(&root, "Migrate Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        std::fs::write(
            &readme_path,
            "# Legacy Hand Authored\n\nCustom orientation a human wrote.\n",
        )
        .expect("write legacy readme");

        let result = refresh_readme(&root, None, false).expect("refresh");
        assert_eq!(result["status"], "saved");
        assert_eq!(result["had_owned_block"], false);
        assert_eq!(
            result["changed_generated_regions"],
            json!(["ownership", "inventory"])
        );
        let after = std::fs::read_to_string(&readme_path).expect("readme after");
        assert!(after.starts_with("# Legacy Hand Authored"));
        assert!(after.contains("Custom orientation a human wrote."));
        assert!(after.contains(crate::pack_readme::README_OWNERSHIP_BEGIN));
        assert!(after.contains(crate::pack_readme::README_INVENTORY_BEGIN));
        // After migration, check reports fresh.
        let check = check_readme(&root).expect("check");
        assert_eq!(check["status"], "fresh");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_repeatedly_refuses_unterminated_fences_without_changing_bytes() {
        let root = std::env::temp_dir().join(format!("mdp-readme-open-fence-{}", nonce()));
        init_pack(&root, "Open Fence Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        for human in [
            "# Human README\n\n```markdown\nkeep backtick bytes\n",
            "# Human README\n\n~~~text\nkeep tilde bytes\n",
            "# Human README\n\n<script>\nkeep raw HTML bytes\n",
            "# Human README\n\n<script\nkeep EOL-opener bytes\n",
            "# Human README\n\n<!--\nkeep comment bytes\n",
            "# Human README\n\n```markdown\nNBSP is not a close\n```\u{00a0}\n",
            "# Human README\n\n~~~text\nem space is not a close\n~~~\u{2003}\n",
            "# Human README\r\r```markdown\rbare CR stays open\r",
        ] {
            std::fs::write(&readme_path, human).expect("write open-fence README");
            let before = check_readme(&root).expect("check legacy open-fence README");
            assert_eq!(before["status"], "unassessed");

            for _ in 0..2 {
                let error = refresh_readme(&root, None, false)
                    .expect_err("refresh must refuse an unsafe EOF insertion");
                assert_eq!(
                    error.to_string(),
                    crate::pack_readme::README_FENCE_DIAGNOSTIC
                );
                assert_eq!(
                    std::fs::read(&readme_path).expect("README after refusal"),
                    human.as_bytes(),
                    "every human byte must survive repeated refusal"
                );
            }

            let after = check_readme(&root).expect("check after refused refresh");
            assert_eq!(after["status"], "unassessed");
            assert_eq!(after["has_ownership_region"], false);
            assert_eq!(after["has_inventory_region"], false);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_markers_fail_repeated_refresh_without_changing_human_bytes() {
        let root = std::env::temp_dir().join(format!("mdp-readme-malformed-{}", nonce()));
        init_pack(&root, "Malformed Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let ownership_begin = crate::pack_readme::README_OWNERSHIP_BEGIN;
        let ownership_end = crate::pack_readme::README_OWNERSHIP_END;
        let inventory_begin = crate::pack_readme::README_INVENTORY_BEGIN;
        let inventory_end = crate::pack_readme::README_INVENTORY_END;
        let cases = [
            format!("# Human\n\nkeep unmatched bytes\n{ownership_begin}\n"),
            format!(
                "# Human\n\n{ownership_begin}\nlegend\n{ownership_end}\nkeep duplicate bytes\n{ownership_begin}\nlegend two\n{ownership_end}\n"
            ),
            format!(
                "# Human\n\n{ownership_begin}\nkeep nested bytes\n{inventory_begin}\ninventory\n{ownership_end}\n{inventory_end}\n"
            ),
        ];
        for malformed in cases {
            std::fs::write(&readme_path, &malformed).expect("write malformed readme");
            for _ in 0..2 {
                let error = refresh_readme(&root, None, false)
                    .expect_err("malformed markers must fail closed");
                assert_eq!(
                    error.to_string(),
                    crate::pack_readme::README_MARKER_DIAGNOSTIC
                );
                assert_eq!(
                    std::fs::read_to_string(&readme_path).expect("readme after refusal"),
                    malformed
                );
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_markers_are_a_stable_validate_blocker() {
        let root = std::env::temp_dir().join(format!("mdp-readme-validate-markers-{}", nonce()));
        init_pack(&root, "Marker Validation Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut readme = std::fs::read_to_string(&readme_path).expect("readme");
        readme.push_str(crate::pack_readme::README_OWNERSHIP_BEGIN);
        std::fs::write(&readme_path, readme).expect("write malformed readme");

        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        assert_eq!(validation["valid"], false);
        assert_eq!(validation["error_count"], 1);
        let marker = validation["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .find(|issue| issue["code"] == "readme_marker_layout_invalid")
            .expect("marker layout diagnostic");
        assert_eq!(marker["severity"], "error");
        assert_eq!(marker["path"], readme_path.display().to_string());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn refresh_preserves_human_only_edits_and_reports_no_generated_change() {
        let root = std::env::temp_dir().join(format!("mdp-readme-human-only-{}", nonce()));
        init_pack(&root, "Human Only Readme Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut edited = std::fs::read_to_string(&readme_path).expect("readme");
        edited.push_str("\nHuman-owned conference note; preserve these exact bytes.\n");
        std::fs::write(&readme_path, &edited).expect("write human edit");

        let result = refresh_readme(&root, None, false).expect("refresh");
        assert_eq!(result["changed_generated_regions"], json!([]));
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("refreshed readme"),
            edited
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn removed_card_and_source_references_warn_without_rewriting_human_prose() {
        let root = std::env::temp_dir().join(format!("mdp-readme-broken-refs-{}", nonce()));
        init_pack(&root, "Broken Reference Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let before = std::fs::read_to_string(&readme_path).expect("readme");

        let mut manifest = read_manifest(&root).expect("manifest");
        let removed_index = manifest
            .cards
            .iter()
            .position(|card| before.contains(&format!("`{}`", card.path)))
            .expect("generated README should reference at least one manifest card");
        let removed_card = manifest.cards.remove(removed_index);
        std::fs::write(
            root.join(".mdp/manifest.yaml"),
            serde_yaml::to_string(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");

        let sources_path = root.join(".mdp/sources.yaml");
        let mut sources: YamlValue =
            serde_yaml::from_str(&std::fs::read_to_string(&sources_path).expect("sources"))
                .expect("parse sources");
        let source_rows = sources["sources"].as_sequence_mut().expect("source rows");
        let removed_source = source_rows[0]["id"].as_str().unwrap().to_string();
        assert!(before.contains(&format!("`{removed_source}`")));
        source_rows.remove(0);
        std::fs::write(
            &sources_path,
            serde_yaml::to_string(&sources).expect("serialize sources"),
        )
        .expect("write sources");

        let result = check_readme(&root).expect("check");
        let warnings = result["warnings"].as_array().expect("warnings");
        assert!(warnings.iter().any(|warning| {
            warning["code"] == "readme_human_card_reference_missing"
                && warning["reference"] == removed_card.path
                && warning["authority"] == "non-authoritative-mechanical-warning"
        }));
        assert!(warnings.iter().any(|warning| {
            warning["code"] == "readme_human_source_reference_missing"
                && warning["reference"] == removed_source
        }));
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("unchanged readme"),
            before,
            "checking references must not rewrite human prose"
        );
        let validation_issues = readme_validation_issues(&root);
        assert!(
            validation_issues
                .iter()
                .any(|warning| { warning["code"] == "readme_human_card_reference_missing" })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn higher_level_heading_ends_sources_warnings_in_check_and_validate() {
        let root = std::env::temp_dir().join(format!("mdp-readme-source-heading-{}", nonce()));
        init_pack(&root, "Source Heading Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut readme = std::fs::read_to_string(&readme_path).expect("README");
        readme.push_str("\n## Sources\n# Other topic\n- `not-a-source`: ordinary prose\n");
        std::fs::write(&readme_path, readme).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert!(
            checked["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|warning| { warning["reference"] != "not-a-source" })
        );
        assert!(
            readme_validation_issues(&root)
                .iter()
                .all(|issue| { issue["reference"] != "not-a-source" })
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn source_reference_parser_ignores_near_headings_inline_code_and_fences() {
        let markdown = r#"# Human README

## Sources appendix
- `near-heading`: must be ignored

```markdown
## Sources
- `fenced-heading`: must be ignored
```

## Sources
Inline `inline-code` must be ignored.

```text
- `fenced-code`: must be ignored
```

- not-backticked: ignored
- `missing-colon` ignored
- `declared-source`: accepted list shape

## Next
- `outside-section`: ignored
"#;
        assert_eq!(source_reference_ids(markdown), vec!["declared-source"]);

        let higher_heading_exit =
            "## Sources\n- `declared`: accepted\n# Other topic\n- `outside-h1`: unrelated\n";
        assert_eq!(
            source_reference_ids(higher_heading_exit),
            vec!["declared"],
            "a higher-level heading ends the Sources section"
        );
        let sibling_title = "## Sources\n- [ref]: /url \"multi\n- `sibling-source`: accepted\n";
        assert_eq!(
            source_reference_ids(sibling_title),
            vec!["sibling-source"],
            "an opened title cannot consume a sibling source item"
        );

        let list_fence = "- ```text\n  list-fenced body\n  ```\n\n## Sources\n- `visible-after-list-fence`: accepted\n";
        assert_eq!(
            source_reference_ids(list_fence),
            vec!["visible-after-list-fence"]
        );
        assert!(
            source_reference_ids(
                "> [ref]: /url\n\"title\"\n2. ```markdown\n   body\n   ```\n## Sources\n- `hidden-after-container-exit`: ignored\n"
            )
            .is_empty(),
            "a pending definition title cannot escape its quote and open the wrong fence"
        );
    }

    #[test]
    fn card_reference_parser_ignores_backtick_and_tilde_fenced_code() {
        let markdown = r#"Human `cards/outside.yaml` reference.

```markdown
`cards/backtick-fenced.yaml`
```

~~~text
`cards/tilde-fenced.yaml`
~~~
"#;
        assert_eq!(
            inline_code_tokens(markdown),
            vec!["cards/outside.yaml"],
            "fenced examples are not human card-reference claims"
        );
        let bare_cr =
            "Outside `cards/outside.yaml`.\r```markdown\r`cards/bare-cr-fenced.yaml`\r```\r";
        assert_eq!(
            inline_code_tokens(bare_cr),
            vec!["cards/outside.yaml"],
            "bare-CR fences must remain block structure"
        );
    }

    #[test]
    fn card_reference_parser_supports_matching_multi_backtick_and_multiline_spans() {
        let markdown = "Double ``cards/double.yaml``. Embedded ``cards/with`tick.yaml``.\nMultiline `\ncards/multiline.yaml\n`.\nMismatched ``cards/not-code.yaml` remains prose.";
        assert_eq!(
            inline_code_tokens(markdown),
            vec![
                "cards/double.yaml",
                "cards/with`tick.yaml",
                "cards/multiline.yaml"
            ]
        );
    }

    #[test]
    fn card_reference_parser_excludes_indented_code_blocks_only() {
        let markdown = "    `cards/first-code.yaml`\n\n\t`cards/continued-code.yaml`\nOutside `cards/visible.yaml`.\nParagraph continuation\n    `cards/paragraph-span.yaml`\n\n- item\n\n    Human `cards/list-continuation.yaml`\n\n>     `cards/blockquote-code.yaml`\n\n-     `cards/list-code.yaml`\n\n10. ordered\n\n    Human `cards/ordered-continuation.yaml`\n\n10.     `cards/ordered-code.yaml`\n";
        assert_eq!(
            inline_code_tokens(markdown),
            vec![
                "cards/visible.yaml",
                "cards/paragraph-span.yaml",
                "cards/list-continuation.yaml",
                "cards/ordered-continuation.yaml"
            ],
            "code blocks respect paragraph, list-container, and blockquote boundaries"
        );
    }

    #[test]
    fn unclosed_container_fences_do_not_hide_root_card_prose() {
        for markdown in [
            "> ```markdown\n> `cards/quoted-hidden.yaml`\nRoot `cards/quoted-visible.yaml`.\n",
            "- ```markdown\n  `cards/list-hidden.yaml`\nRoot `cards/list-visible.yaml`.\n",
            "- > ```markdown\n  > `cards/nested-hidden.yaml`\nRoot `cards/nested-visible.yaml`.\n",
            "- outer\n  - ```markdown\n    `cards/nested-list-hidden.yaml`\nRoot `cards/nested-list-visible.yaml`.\n",
        ] {
            let tokens = inline_code_tokens(markdown);
            assert_eq!(
                tokens.len(),
                1,
                "markdown: {markdown:?}; tokens: {tokens:?}"
            );
            assert!(tokens[0].ends_with("-visible.yaml"), "tokens: {tokens:?}");
        }
        assert_eq!(
            inline_code_tokens(
                "- ```markdown\n `cards/underindented-visible.yaml`\nRoot `cards/root-visible.yaml`.\n"
            ),
            vec![
                "cards/underindented-visible.yaml",
                "cards/root-visible.yaml"
            ],
            "a bullet continuation includes marker width plus padding"
        );
        assert_eq!(
            inline_code_tokens(
                "> quoted paragraph\n2. ```markdown\n   `cards/exited-paragraph-hidden.yaml`\n   ```\nRoot `cards/exited-paragraph-visible.yaml`.\n"
            ),
            vec!["cards/exited-paragraph-visible.yaml"],
            "leaving a container clears its paragraph before parsing a root list"
        );
        assert_eq!(
            inline_code_tokens(
                "[ref]: /url \"multi\nline\"\n2. ```markdown\n   `cards/multiline-title-hidden.yaml`\n   ```\nRoot `cards/multiline-title-visible.yaml`.\n"
            ),
            vec!["cards/multiline-title-visible.yaml"],
            "multiline definition titles close before later root blocks"
        );
        assert_eq!(
            inline_code_tokens("- [ref]: /url \"multi\n- `cards/sibling-title-visible.yaml`\"\n"),
            vec!["cards/sibling-title-visible.yaml"],
            "an opened title cannot consume a sibling list item"
        );
        assert_eq!(
            inline_code_tokens("- paragraph\n<x>\nRoot `cards/lazy-list-visible.yaml`.\n"),
            vec!["cards/lazy-list-visible.yaml"],
            "type-7 HTML stays a lazy list-paragraph continuation"
        );
        assert_eq!(
            inline_code_tokens(
                "Paragraph\n2. ```markdown\n   `cards/non-one-ordered-visible.yaml`\n"
            ),
            vec!["cards/non-one-ordered-visible.yaml"],
            "an ordered list not starting at one cannot interrupt a paragraph"
        );
        assert!(
            inline_code_tokens("Paragraph\n1. ```markdown\n   `cards/one-ordered-hidden.yaml`\n")
                .is_empty(),
            "an ordered list starting at one may interrupt a paragraph"
        );
        assert_eq!(
            inline_code_tokens(
                "# Heading\n2. ```markdown\n   `cards/after-heading-hidden.yaml`\n   ```\nRoot `cards/after-heading-visible.yaml`.\n"
            ),
            vec!["cards/after-heading-visible.yaml"],
            "an ATX heading closes the prior block before an ordered list"
        );
        assert_eq!(
            inline_code_tokens(
                "[ref]: /url\n2. ```markdown\n   `cards/after-definition-hidden.yaml`\n   ```\nRoot `cards/after-definition-visible.yaml`.\n"
            ),
            vec!["cards/after-definition-visible.yaml"],
            "a link reference definition closes before an ordered list"
        );
        assert_eq!(
            inline_code_tokens(
                "[ref]: <foo bar>\n2. ```markdown\n   `cards/invalid-definition-visible.yaml`\n   ```\n"
            ),
            vec!["cards/invalid-definition-visible.yaml"],
            "a definition-shaped invalid destination remains paragraph text"
        );
        assert_eq!(
            inline_code_tokens(
                "[ref]: /url\n  \"title\"\n2. ```markdown\n   `cards/multiline-definition-hidden.yaml`\n   ```\nRoot `cards/multiline-definition-visible.yaml`.\n"
            ),
            vec!["cards/multiline-definition-visible.yaml"],
            "an optional title continuation remains part of the definition block"
        );
        assert_eq!(
            inline_code_tokens(
                "> [ref]: /url\n>   \"title\"\n> 2. ```markdown\n>    `cards/quoted-definition-hidden.yaml`\n>    ```\nRoot `cards/quoted-definition-visible.yaml`.\n"
            ),
            vec!["cards/quoted-definition-visible.yaml"],
            "definition title continuations are projected through their container"
        );
        assert_eq!(
            inline_code_tokens(
                "> [ref]: /url\n\"title\"\n2. ```markdown\n   `cards/exited-definition-visible.yaml`\n   ```\nRoot `cards/exited-definition-hidden.yaml`.\n"
            ),
            vec!["cards/exited-definition-visible.yaml"],
            "a definition title continuation cannot escape its opening container"
        );
        assert_eq!(
            inline_code_tokens(
                "- [ref]: /url\n- \"title\"\n  2. ```markdown\n     `cards/sibling-definition-visible.yaml`\n     ```\n"
            ),
            vec!["cards/sibling-definition-visible.yaml"],
            "a definition title continuation cannot move to a sibling list item"
        );
        assert_eq!(
            inline_code_tokens(
                "[ref]:\n  /url\n2. ```markdown\n   `cards/destination-continuation-hidden.yaml`\n   ```\nRoot `cards/destination-continuation-visible.yaml`.\n"
            ),
            vec!["cards/destination-continuation-visible.yaml"],
            "a continued link destination completes the definition block"
        );
        assert_eq!(
            inline_code_tokens(
                "Paragraph\n* \n2. ```markdown\n   `cards/after-empty-item-visible.yaml`\n   ```\nRoot `cards/after-empty-item-hidden.yaml`.\n"
            ),
            vec!["cards/after-empty-item-visible.yaml"],
            "an empty list item cannot interrupt an open paragraph"
        );
        assert_eq!(
            inline_code_tokens(
                "Heading\n-\n2. ```markdown\n   `cards/after-short-setext-hidden.yaml`\n   ```\nRoot `cards/after-short-setext-visible.yaml`.\n"
            ),
            vec!["cards/after-short-setext-visible.yaml"],
            "a single hyphen is a valid setext underline"
        );
        assert_eq!(
            inline_code_tokens(
                "Heading\n-   \n2. ```markdown\n   `cards/after-spaced-setext-hidden.yaml`\n   ```\nRoot `cards/after-spaced-setext-visible.yaml`.\n"
            ),
            vec!["cards/after-spaced-setext-visible.yaml"],
            "a short setext underline permits trailing whitespace"
        );
        assert_eq!(
            inline_code_tokens(
                "Paragraph\n[ref]: /url\n2. ```markdown\n   `cards/paragraph-definition-visible.yaml`\n   ```\nRoot `cards/paragraph-definition-hidden.yaml`.\n"
            ),
            vec!["cards/paragraph-definition-visible.yaml"],
            "a link definition cannot interrupt an open paragraph"
        );
        assert_eq!(
            inline_code_tokens(
                "=\n2. ```markdown\n   `cards/standalone-equals-visible.yaml`\n   ```\nRoot `cards/standalone-equals-hidden.yaml`.\n"
            ),
            vec!["cards/standalone-equals-visible.yaml"],
            "a standalone equals line is paragraph content, not a setext underline"
        );
        assert_eq!(
            inline_code_tokens(
                "[ ]: /url\n2. ```markdown\n   `cards/blank-label-visible.yaml`\n   ```\nRoot `cards/blank-label-hidden.yaml`.\n"
            ),
            vec!["cards/blank-label-visible.yaml"],
            "a whitespace-only reference label is invalid paragraph content"
        );
        assert!(
            inline_code_tokens("# Example\n    `cards/heading-indented-code.yaml`\n").is_empty(),
            "indented code may begin immediately after a heading"
        );
        assert!(
            inline_code_tokens(
                "[ref]: /url\n  \"title\"\n    `cards/definition-indented-code.yaml`\n"
            )
            .is_empty(),
            "indented code may begin immediately after a definition continuation"
        );
        assert!(
            inline_code_tokens("[ref]: /url\n  \"See `cards/definition-title.yaml`\"\n").is_empty(),
            "link-definition titles are not inline-parsed"
        );
        assert_eq!(
            inline_code_tokens(
                "<script>\n`cards/raw-html.yaml`\n</script>\nRoot `cards/raw-root.yaml`.\n"
            ),
            vec!["cards/raw-root.yaml"],
            "raw HTML block content is not inline-parsed"
        );
        assert_eq!(
            inline_code_tokens(
                "<script\n`cards/raw-eol-html.yaml`\n</script>\nRoot `cards/raw-eol-root.yaml`.\n"
            ),
            vec!["cards/raw-eol-root.yaml"],
            "a type-1 raw HTML opener may end at EOL"
        );
        assert_eq!(
            inline_code_tokens(
                "```markdown\n<script>\n```\nRoot `cards/fenced-script-root.yaml`.\n"
            ),
            vec!["cards/fenced-script-root.yaml"],
            "raw HTML state cannot start inside fenced code"
        );
        assert_eq!(
            inline_code_tokens(
                "<?php\n`cards/processing-instruction.yaml`\n?>\nRoot `cards/processing-root.yaml`.\n"
            ),
            vec!["cards/processing-root.yaml"],
            "processing-instruction raw HTML is not inline-parsed"
        );
        assert_eq!(
            inline_code_tokens(
                "<widget data-value='human'>\n`cards/custom-html.yaml`\n\nRoot `cards/custom-root.yaml`.\n"
            ),
            vec!["cards/custom-root.yaml"],
            "complete custom-tag raw HTML is not inline-parsed"
        );
        assert_eq!(
            inline_code_tokens(
                "> <div>\nRoot `cards/after-quoted-html.yaml`.\n- <div>\nRoot `cards/after-list-html.yaml`.\n"
            ),
            vec!["cards/after-quoted-html.yaml", "cards/after-list-html.yaml"],
            "raw HTML state ends when its quote or list container exits"
        );
        assert_eq!(
            inline_code_tokens("<![cdata[\n`cards/lowercase-cdata-visible.yaml`\n]]>\n"),
            vec!["cards/lowercase-cdata-visible.yaml"],
            "CDATA openers are case-sensitive"
        );
        assert_eq!(
            inline_code_tokens("<x:y>\n`cards/unseparated-attribute-visible.yaml`\n"),
            vec!["cards/unseparated-attribute-visible.yaml"],
            "HTML attributes require separating whitespace"
        );
        assert_eq!(
            inline_code_tokens("<div/x\n`cards/invalid-slash-visible.yaml`\n"),
            vec!["cards/invalid-slash-visible.yaml"],
            "a slash is a block-tag boundary only as part of />"
        );
    }

    #[test]
    fn readme_check_warns_for_card_after_definition_sibling() {
        let root = std::env::temp_dir().join(format!("mdp-readme-definition-sibling-{}", nonce()));
        init_pack(&root, "Definition Sibling Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut readme = std::fs::read_to_string(&readme_path).expect("README");
        readme.push_str(
            "\n- [ref]: /url\n- \"title\"\n  2. ```markdown\n     `cards/sibling-definition-visible.yaml`\n     ```\n",
        );
        std::fs::write(&readme_path, readme).expect("write sibling definition fixture");

        let checked = check_readme(&root).expect("check README");
        assert!(
            checked["warnings"]
                .as_array()
                .expect("warnings")
                .iter()
                .any(
                    |warning| warning["code"] == "readme_human_card_reference_missing"
                        && warning["reference"] == "cards/sibling-definition-visible.yaml"
                )
        );
        assert!(readme_validation_issues(&root).iter().any(|warning| {
            warning["code"] == "readme_human_card_reference_missing"
                && warning["reference"] == "cards/sibling-definition-visible.yaml"
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn blockquote_entry_resets_paragraph_state_for_indented_code() {
        let markdown =
            "Paragraph\n>     `cards/quoted-code.yaml`\nRoot `cards/root-visible.yaml`.\n";
        assert_eq!(
            inline_code_tokens(markdown),
            vec!["cards/root-visible.yaml"]
        );
    }

    #[test]
    fn readme_check_and_validate_ignore_indented_code_card_examples() {
        let root = std::env::temp_dir().join(format!("mdp-readme-indented-code-{}", nonce()));
        init_pack(&root, "Indented Code Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let mut readme = std::fs::read_to_string(&readme_path).expect("README");
        readme.push_str(
            "\n\n    Example `cards/indented-missing.yaml`\n\n    Continued `cards/continued-missing.yaml`\n\n- item\n\n    Human `cards/list-missing.yaml`\n\n>     `cards/blockquote-missing.yaml`\n\n> ```markdown\n> `cards/quoted-fenced.yaml`\nRoot `cards/quoted-root.yaml`.\n\n- ```markdown\n  `cards/list-fenced.yaml`\nRoot `cards/list-root.yaml`.\n\n- outer\n  - ```markdown\n    `cards/nested-list-fenced.yaml`\nRoot `cards/nested-list-root.yaml`.\n\nParagraph\n2. ```markdown\n   `cards/non-one-ordered-visible.yaml`\n\n# Heading\n2. ```markdown\n   `cards/after-heading-hidden.yaml`\n   ```\nRoot `cards/after-heading-visible.yaml`.\n\n[ref]: /url\n2. ```markdown\n   `cards/after-definition-hidden.yaml`\n   ```\nRoot `cards/after-definition-visible.yaml`.\n\n> [ref]: /url\n>   \"title\"\n> 2. ```markdown\n>    `cards/quoted-definition-hidden.yaml`\n>    ```\nRoot `cards/quoted-definition-visible.yaml`.\n\nHuman `cards/visible-missing.yaml`.\n",
        );
        readme.push_str(&format!(
            "\n[title-ref]: /url\n  \"See `cards/definition-title-missing.yaml`\"\n\n<script>\n`cards/raw-html-missing.yaml`\n{}\nhuman raw marker\n{}\n{}\nhuman raw marker\n{}\n</script>\n<?php\n`cards/raw-processing-missing.yaml`\n?>\n<![CDATA[\n`cards/raw-cdata-missing.yaml`\n]]>\n<div>\n`cards/raw-block-missing.yaml`\n\n<div\n`cards/raw-block-eol-missing.yaml`\n\n<widget data-value='human'>\n`cards/raw-custom-missing.yaml`\n\n",
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            crate::pack_readme::README_OWNERSHIP_END,
            crate::pack_readme::README_INVENTORY_BEGIN,
            crate::pack_readme::README_INVENTORY_END,
        ));
        std::fs::write(&readme_path, readme).expect("write human examples");
        refresh_readme(&root, None, false).expect("refresh with raw HTML examples");
        let refreshed = std::fs::read_to_string(&readme_path).expect("refreshed README");
        for preserved in [
            "`cards/raw-html-missing.yaml`",
            "`cards/raw-processing-missing.yaml`",
            "`cards/raw-cdata-missing.yaml`",
            "`cards/raw-block-missing.yaml`",
            "`cards/raw-block-eol-missing.yaml`",
            "`cards/raw-custom-missing.yaml`",
        ] {
            assert!(
                refreshed.contains(preserved),
                "refresh preserves {preserved}"
            );
        }

        let checked = check_readme(&root).expect("check README");
        let check_refs = checked["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .filter(|warning| warning["code"] == "readme_human_card_reference_missing")
            .map(|warning| warning["reference"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            check_refs,
            vec![
                "cards/list-missing.yaml",
                "cards/quoted-root.yaml",
                "cards/list-root.yaml",
                "cards/nested-list-root.yaml",
                "cards/non-one-ordered-visible.yaml",
                "cards/after-heading-visible.yaml",
                "cards/after-definition-visible.yaml",
                "cards/quoted-definition-visible.yaml",
                "cards/visible-missing.yaml"
            ]
        );

        let validate_refs = readme_validation_issues(&root)
            .into_iter()
            .filter(|issue| issue["code"] == "readme_human_card_reference_missing")
            .map(|issue| issue["reference"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            validate_refs,
            vec![
                "cards/list-missing.yaml",
                "cards/quoted-root.yaml",
                "cards/list-root.yaml",
                "cards/nested-list-root.yaml",
                "cards/non-one-ordered-visible.yaml",
                "cards/after-heading-visible.yaml",
                "cards/after-definition-visible.yaml",
                "cards/quoted-definition-visible.yaml",
                "cards/visible-missing.yaml"
            ]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn four_space_html_opener_does_not_hide_or_duplicate_owned_regions() {
        let root = std::env::temp_dir().join(format!("mdp-readme-indented-html-{}", nonce()));
        init_pack(&root, "Indented HTML Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README");
        let with_indented_opener = readme.replacen(
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            &format!(
                "    <script>\n{}",
                crate::pack_readme::README_OWNERSHIP_BEGIN
            ),
            1,
        ) + "</script>\n";
        std::fs::write(&readme_path, &with_indented_opener).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert_eq!(checked["status"], "fresh", "{checked}");
        refresh_readme(&root, None, false).expect("refresh README");
        let refreshed = std::fs::read_to_string(&readme_path).expect("refreshed README");
        assert_eq!(
            refreshed
                .matches(crate::pack_readme::README_OWNERSHIP_BEGIN)
                .count(),
            1
        );
        assert_eq!(
            refreshed
                .matches(crate::pack_readme::README_INVENTORY_BEGIN)
                .count(),
            1
        );
        assert!(refreshed.contains("    <script>\n"));
        assert!(refreshed.ends_with("</script>\n"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn lowercase_cdata_does_not_hide_or_duplicate_owned_regions() {
        let root = std::env::temp_dir().join(format!("mdp-readme-lowercase-cdata-{}", nonce()));
        init_pack(&root, "Lowercase CDATA Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README");
        let invalid_cdata = readme.replacen(
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            &format!("<![cdata[\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
            1,
        ) + "]]>\n";
        std::fs::write(&readme_path, &invalid_cdata).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert_eq!(checked["status"], "fresh", "{checked}");
        refresh_readme(&root, None, false).expect("refresh README");
        let refreshed = std::fs::read_to_string(&readme_path).expect("refreshed README");
        assert_eq!(
            refreshed
                .matches(crate::pack_readme::README_OWNERSHIP_BEGIN)
                .count(),
            1
        );
        assert_eq!(
            refreshed
                .matches(crate::pack_readme::README_INVENTORY_BEGIN)
                .count(),
            1
        );
        assert!(refreshed.contains("<![cdata[\n"));
        assert!(refreshed.ends_with("]]>\n"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn multiline_definition_and_lazy_list_prose_keep_check_refresh_consistent() {
        for human_prefix in [
            "[ref]: /url \"multi\nline\"\n2. ```markdown\n   human body\n   ```",
            "- paragraph\n<x>",
        ] {
            let root = std::env::temp_dir().join(format!("mdp-readme-block-state-{}", nonce()));
            init_pack(&root, "Block State Pack", "gtm", true, false)
                .expect("pack should initialize");
            let readme_path = root.join(".mdp/README.md");
            let readme = std::fs::read_to_string(&readme_path).expect("README");
            let adversarial = readme.replacen(
                crate::pack_readme::README_OWNERSHIP_BEGIN,
                &format!(
                    "{human_prefix}\n{}",
                    crate::pack_readme::README_OWNERSHIP_BEGIN
                ),
                1,
            );
            std::fs::write(&readme_path, &adversarial).expect("write README");

            let checked = check_readme(&root).expect("check README");
            assert_eq!(checked["status"], "fresh", "{human_prefix}: {checked}");
            let dry_run = refresh_readme(&root, None, true).expect("dry-run refresh");
            assert_eq!(dry_run["status"], "dry-run");
            assert_eq!(
                std::fs::read_to_string(&readme_path).expect("README after dry run"),
                adversarial
            );
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn invalid_multiline_title_restores_paragraph_and_hides_owned_regions() {
        let root = std::env::temp_dir().join(format!("mdp-readme-invalid-title-{}", nonce()));
        init_pack(&root, "Invalid Title Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README");
        let prefix = "[ref]: /url \"multi\n    line\"\n2. ```markdown\n   human paragraph\n   ```";
        let adversarial = readme.replacen(
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            &format!("{prefix}\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
            1,
        );
        std::fs::write(&readme_path, &adversarial).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert_eq!(checked["status"], "unassessed", "{checked}");
        let error = refresh_readme(&root, None, true)
            .expect_err("dry-run must refuse insertion inside the resulting open fence");
        assert_eq!(
            error.to_string(),
            crate::pack_readme::README_FENCE_DIAGNOSTIC
        );
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("README after refusal"),
            adversarial
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn multiline_title_tracks_its_actual_opening_delimiter() {
        let root = std::env::temp_dir().join(format!("mdp-readme-title-delimiter-{}", nonce()));
        init_pack(&root, "Title Delimiter Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README");
        let prefix = "[ref]: /url \"multi '\nline\"\n2. ```markdown\n   human fence body\n   ```";
        let adversarial = readme.replacen(
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            &format!("{prefix}\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
            1,
        );
        std::fs::write(&readme_path, &adversarial).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert_eq!(checked["status"], "fresh", "{checked}");
        let dry_run = refresh_readme(&root, None, true).expect("dry-run refresh");
        assert_eq!(dry_run["status"], "dry-run");
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("README after dry run"),
            adversarial
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn escaped_destination_space_keeps_check_refresh_fail_closed() {
        let root = std::env::temp_dir().join(format!("mdp-readme-invalid-destination-{}", nonce()));
        init_pack(&root, "Invalid Destination Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README");
        let prefix = "[ref]: /url\\ space\n2. ```markdown\n\n   ```";
        let adversarial = readme.replacen(
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            &format!("{prefix}\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
            1,
        );
        std::fs::write(&readme_path, &adversarial).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert_eq!(checked["status"], "unassessed", "{checked}");
        let error = refresh_readme(&root, None, true)
            .expect_err("dry-run must refuse insertion inside the actual open fence");
        assert_eq!(
            error.to_string(),
            crate::pack_readme::README_FENCE_DIAGNOSTIC
        );
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("README after refusal"),
            adversarial
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unicode_reference_label_limit_keeps_check_refresh_aligned() {
        let root = std::env::temp_dir().join(format!("mdp-readme-unicode-label-{}", nonce()));
        init_pack(&root, "Unicode Label Pack", "gtm", true, false).expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("README");
        let label = "é".repeat(999);
        let prefix = format!("[{label}]: /url\n2. ```markdown\n   human fence body\n   ```");
        let adversarial = readme.replacen(
            crate::pack_readme::README_OWNERSHIP_BEGIN,
            &format!("{prefix}\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
            1,
        );
        std::fs::write(&readme_path, &adversarial).expect("write README");

        let checked = check_readme(&root).expect("check README");
        assert_eq!(checked["status"], "fresh", "{checked}");
        let dry_run = refresh_readme(&root, None, true).expect("dry-run refresh");
        assert_eq!(dry_run["status"], "dry-run");
        assert_eq!(
            std::fs::read_to_string(&readme_path).expect("README after dry run"),
            adversarial
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_html_tag_shapes_do_not_hide_owned_regions() {
        for invalid in ["<x:y>", "<div/x"] {
            let root = std::env::temp_dir().join(format!(
                "mdp-readme-invalid-html-{}-{}",
                invalid.len(),
                nonce()
            ));
            init_pack(&root, "Invalid HTML Pack", "gtm", true, false)
                .expect("pack should initialize");
            let readme_path = root.join(".mdp/README.md");
            let readme = std::fs::read_to_string(&readme_path).expect("README");
            let invalid_html = readme.replacen(
                crate::pack_readme::README_OWNERSHIP_BEGIN,
                &format!("{invalid}\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
                1,
            );
            std::fs::write(&readme_path, &invalid_html).expect("write README");

            let checked = check_readme(&root).expect("check README");
            assert_eq!(checked["status"], "fresh", "{invalid}: {checked}");
            refresh_readme(&root, None, false).expect("refresh README");
            let refreshed = std::fs::read_to_string(&readme_path).expect("refreshed README");
            assert_eq!(
                refreshed
                    .matches(crate::pack_readme::README_OWNERSHIP_BEGIN)
                    .count(),
                1
            );
            assert_eq!(
                refreshed
                    .matches(crate::pack_readme::README_INVENTORY_BEGIN)
                    .count(),
                1
            );
            assert!(refreshed.contains(&format!("{invalid}\n")));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn contained_html_opener_does_not_hide_root_owned_regions() {
        for opener in ["> <div>", "- <div>"] {
            let root = std::env::temp_dir().join(format!(
                "mdp-readme-contained-html-{}-{}",
                opener.as_bytes()[0],
                nonce()
            ));
            init_pack(&root, "Contained HTML Pack", "gtm", true, false)
                .expect("pack should initialize");
            let readme_path = root.join(".mdp/README.md");
            let readme = std::fs::read_to_string(&readme_path).expect("README");
            let with_contained_opener = readme.replacen(
                crate::pack_readme::README_OWNERSHIP_BEGIN,
                &format!("{opener}\n{}", crate::pack_readme::README_OWNERSHIP_BEGIN),
                1,
            );
            std::fs::write(&readme_path, &with_contained_opener).expect("write README");

            let checked = check_readme(&root).expect("check README");
            assert_eq!(checked["status"], "fresh", "{opener}: {checked}");
            refresh_readme(&root, None, false).expect("refresh README");
            let refreshed = std::fs::read_to_string(&readme_path).expect("refreshed README");
            assert_eq!(
                refreshed
                    .matches(crate::pack_readme::README_OWNERSHIP_BEGIN)
                    .count(),
                1
            );
            assert_eq!(
                refreshed
                    .matches(crate::pack_readme::README_INVENTORY_BEGIN)
                    .count(),
                1
            );
            assert!(refreshed.contains(&format!("{opener}\n")));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    #[test]
    fn card_reference_membership_normalizes_safe_relative_paths_only() {
        let root = std::env::temp_dir().join(format!("mdp-readme-card-paths-{}", nonce()));
        init_pack(&root, "Card Path Pack", "gtm", true, false).expect("pack should initialize");
        let mut manifest = read_manifest(&root).expect("manifest");
        manifest.cards[0].path = "./cards/equivalent.yaml".to_string();
        let readme = "Canonical `cards/equivalent.yaml`; missing `./cards/missing.yaml`; reject `cards/../escape.yaml` and `/cards/absolute.yaml`.";
        let warnings = human_reference_warnings(
            readme,
            &manifest,
            &Value::Null,
            &root.join(".mdp/README.md"),
        );
        let card_warnings = warnings
            .iter()
            .filter(|warning| warning["code"] == "readme_human_card_reference_missing")
            .collect::<Vec<_>>();
        assert_eq!(card_warnings.len(), 1);
        assert_eq!(card_warnings[0]["reference"], "./cards/missing.yaml");
        assert_eq!(
            normalize_card_path("./cards/equivalent.yaml").as_deref(),
            Some("cards/equivalent.yaml")
        );
        assert!(normalize_card_path("cards/../escape.yaml").is_none());
        assert!(normalize_card_path("/cards/absolute.yaml").is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_backtick_fence_info_does_not_hide_card_references() {
        let markdown = "```markdown`invalid\n``cards/visible.yaml``\n";
        // The invalid opener is ordinary prose rather than a block fence. Its
        // unmatched delimiter runs do not suppress a later valid code span.
        assert!(
            inline_code_tokens(markdown)
                .iter()
                .any(|token| token == "cards/visible.yaml")
        );
    }

    #[test]
    fn validate_pack_reports_drift_as_warning_that_strict_promotes_to_blocker() {
        let root = std::env::temp_dir().join(format!("mdp-readme-validate-{}", nonce()));
        init_pack(&root, "Validate Drift Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("readme");
        let corrupted = readme.replace("- cards:", "- card count:");
        std::fs::write(&readme_path, &corrupted).expect("write corrupted readme");

        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        // Non-strict validate treats drift as a warning so legacy and
        // card-mutating flows remain compatible; strict validation promotes it
        // to a blocker via apply_strict.
        assert_eq!(validation["valid"], true);
        let issues = validation["issues"].as_array().expect("issues array");
        let drift = issues
            .iter()
            .find(|issue| issue["code"] == "readme_inventory_drift")
            .expect("validate should report readme drift");
        assert_eq!(drift["severity"], "warning");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn validate_pack_leaves_legacy_readme_unassessed_without_blocking() {
        let root = std::env::temp_dir().join(format!("mdp-readme-validate-legacy-{}", nonce()));
        init_pack(&root, "Validate Legacy Pack", "gtm", true, false)
            .expect("pack should initialize");
        let readme_path = root.join(".mdp/README.md");
        std::fs::write(&readme_path, "# Legacy\n\nNo marker here.\n").expect("write legacy");
        let validation = crate::commands::health::validate_pack(&root).expect("validate");
        let issues = validation["issues"].as_array().expect("issues");
        assert!(
            !issues
                .iter()
                .any(|issue| issue["code"] == "readme_inventory_drift"),
            "legacy README must not produce a drift issue"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
