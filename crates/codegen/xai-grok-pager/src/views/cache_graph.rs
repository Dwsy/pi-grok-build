//! Context-modal cache graph/stats — pi-cache-graph formula and layout.
//!
//! Pure rendering over [`CacheSessionMetrics`]; no ACP or Pi RPC.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use xai_grok_shell::session::{AssistantUsageMetric, CacheSessionMetrics, CacheUsageTotals};

use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheGraphView {
    /// Existing ContextInfoBlock breakdown.
    #[default]
    Breakdown,
    /// Per-turn cache hit %.
    PerTurn,
    /// Cumulative aggregate hit %.
    CumulativePercent,
    /// Cumulative token volumes.
    CumulativeTotal,
    /// Per-message stats table.
    Stats,
}

impl CacheGraphView {
    pub fn title_suffix(self) -> &'static str {
        match self {
            Self::Breakdown => "Context",
            Self::PerTurn => "Cache — Per-turn (%)",
            Self::CumulativePercent => "Cache — Cumulative %",
            Self::CumulativeTotal => "Cache — Cumulative total",
            Self::Stats => "Cache — Stats",
        }
    }

    pub fn cycle_forward(self) -> Self {
        match self {
            Self::Breakdown => Self::PerTurn,
            Self::PerTurn => Self::CumulativePercent,
            Self::CumulativePercent => Self::CumulativeTotal,
            Self::CumulativeTotal => Self::Stats,
            Self::Stats => Self::Breakdown,
        }
    }
}

pub fn format_int(value: u64) -> String {
    let s = value.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

pub fn format_percent(value: f64) -> String {
    format!("{value:.1}%")
}

pub fn summarize_hit_percent(totals: &CacheUsageTotals) -> f64 {
    let den = totals
        .input
        .saturating_add(totals.cache_read)
        .saturating_add(totals.cache_write);
    if den == 0 {
        0.0
    } else {
        (totals.cache_read as f64 / den as f64) * 100.0
    }
}

fn format_totals_line(label: &str, totals: &CacheUsageTotals) -> String {
    format!(
        "{label}: {} turns • prompt {} • received {} • cache hit {} • cache write {} • hit rate {}",
        format_int(totals.assistant_messages),
        format_int(
            totals
                .input
                .saturating_add(totals.cache_read)
                .saturating_add(totals.cache_write)
        ),
        format_int(totals.output),
        format_int(totals.cache_read),
        format_int(totals.cache_write),
        format_percent(summarize_hit_percent(totals)),
    )
}

fn dim_line(theme: &Theme, text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().fg(theme.gray_dim),
    ))
}

fn muted_line(theme: &Theme, text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), theme.muted()))
}

fn accent_line(theme: &Theme, text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().fg(theme.text_primary),
    ))
}

fn warning_line(theme: &Theme, text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(
        text.into(),
        Style::default().fg(theme.warning),
    ))
}

struct CumSeries {
    cum_input: Vec<u64>,
    cum_cache_read: Vec<u64>,
    cum_cache_write: Vec<u64>,
    cum_hit_percent: Vec<f64>,
}

fn compute_cumulative(messages: &[AssistantUsageMetric]) -> CumSeries {
    let mut cum_input = Vec::with_capacity(messages.len());
    let mut cum_cache_read = Vec::with_capacity(messages.len());
    let mut cum_cache_write = Vec::with_capacity(messages.len());
    let mut cum_hit_percent = Vec::with_capacity(messages.len());
    let mut sum_in = 0u64;
    let mut sum_read = 0u64;
    let mut sum_write = 0u64;
    for m in messages {
        sum_in = sum_in.saturating_add(m.input);
        sum_read = sum_read.saturating_add(m.cache_read);
        sum_write = sum_write.saturating_add(m.cache_write);
        cum_input.push(sum_in);
        cum_cache_read.push(sum_read);
        cum_cache_write.push(sum_write);
        let den = sum_in.saturating_add(sum_read).saturating_add(sum_write);
        cum_hit_percent.push(if den == 0 {
            0.0
        } else {
            (sum_read as f64 / den as f64) * 100.0
        });
    }
    CumSeries {
        cum_input,
        cum_cache_read,
        cum_cache_write,
        cum_hit_percent,
    }
}

fn bucket_messages(
    messages: &[AssistantUsageMetric],
    bucket_count: usize,
) -> Vec<&[AssistantUsageMetric]> {
    if messages.is_empty() || bucket_count == 0 {
        return vec![];
    }
    if messages.len() <= bucket_count {
        return messages.iter().map(std::slice::from_ref).collect();
    }
    let mut buckets = Vec::with_capacity(bucket_count);
    for i in 0..bucket_count {
        let start = (i * messages.len()) / bucket_count;
        let end = ((i + 1) * messages.len()) / bucket_count;
        let end = end.max(start + 1);
        buckets.push(&messages[start..end.min(messages.len())]);
    }
    buckets
}

fn bucket_max(values: &[f64], bucket_count: usize) -> Vec<f64> {
    if values.is_empty() || bucket_count == 0 {
        return vec![];
    }
    if values.len() <= bucket_count {
        return values.to_vec();
    }
    let mut out = Vec::with_capacity(bucket_count);
    for i in 0..bucket_count {
        let start = (i * values.len()) / bucket_count;
        let end = ((i + 1) * values.len()) / bucket_count;
        let end = end.max(start + 1).min(values.len());
        let max = values[start..end]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        out.push(max);
    }
    out
}

fn bucket_max_u64(values: &[u64], bucket_count: usize) -> Vec<u64> {
    if values.is_empty() || bucket_count == 0 {
        return vec![];
    }
    if values.len() <= bucket_count {
        return values.to_vec();
    }
    let mut out = Vec::with_capacity(bucket_count);
    for i in 0..bucket_count {
        let start = (i * values.len()) / bucket_count;
        let end = ((i + 1) * values.len()) / bucket_count;
        let end = end.max(start + 1).min(values.len());
        out.push(values[start..end].iter().copied().max().unwrap_or(0));
    }
    out
}

fn average_hit(messages: &[AssistantUsageMetric]) -> f64 {
    if messages.is_empty() {
        return 0.0;
    }
    messages.iter().map(|m| m.cache_hit_percent).sum::<f64>() / messages.len() as f64
}

fn render_bar_chart(theme: &Theme, values: &[f64], chart_height: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(chart_height + 1);
    let muted = theme.muted();
    let accent = Style::default().fg(theme.text_primary);
    let dim = Style::default().fg(theme.gray_dim);
    for row in (1..=chart_height).rev() {
        let threshold = (row as f64 / chart_height as f64) * 100.0;
        let label = format!("{:3}│", threshold.round() as i32);
        let mut spans = vec![Span::styled(label, muted)];
        for &v in values {
            spans.push(Span::styled(
                if v >= threshold { "█" } else { "·" }.to_string(),
                if v >= threshold { accent } else { dim },
            ));
        }
        lines.push(Line::from(spans));
    }
    let base = format!("  0│{}", "─".repeat(values.len()));
    lines.push(Line::from(Span::styled(base, muted)));
    lines
}

fn render_stacked_chart(
    theme: &Theme,
    series_input: &[u64],
    series_read: &[u64],
    series_write: &[u64],
    chart_height: usize,
) -> (Vec<Line<'static>>, u64) {
    let max_val = series_input
        .iter()
        .chain(series_read.iter())
        .chain(series_write.iter())
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);
    let mut unit_tokens = 5000u64;
    if max_val > (chart_height as u64).saturating_mul(unit_tokens) {
        unit_tokens = ((max_val / chart_height as u64 + 4999) / 5000) * 5000;
        unit_tokens = unit_tokens.max(5000);
    }
    let muted = theme.muted();
    let accent = Style::default().fg(theme.accent_skill);
    let warning = Style::default().fg(theme.warning);
    let input_style = Style::default().fg(theme.gray_bright);
    let dim = Style::default().fg(theme.gray_dim);
    let mut lines = Vec::with_capacity(chart_height + 1);
    for row in (1..=chart_height).rev() {
        let threshold = (row as u64).saturating_mul(unit_tokens);
        let k_val = threshold / 1000;
        let label = format!("{:3}k│", k_val);
        let mut spans = vec![Span::styled(label, muted)];
        for i in 0..series_input.len() {
            let inp = series_input[i];
            let read = series_read[i];
            let write = series_write[i];
            let (ch, style) = if read >= threshold {
                ("▒", accent)
            } else if write >= threshold {
                ("░", warning)
            } else if inp >= threshold {
                ("▇", input_style)
            } else {
                ("·", dim)
            };
            spans.push(Span::styled(ch.to_string(), style));
        }
        lines.push(Line::from(spans));
    }
    let base = format!("    0│{}", "─".repeat(series_input.len()));
    lines.push(Line::from(Span::styled(base, muted)));
    (lines, unit_tokens)
}

/// Render graph body lines for a cache view (not Breakdown).
pub fn render_cache_view_lines(
    theme: &Theme,
    metrics: &CacheSessionMetrics,
    width: u16,
    view: CacheGraphView,
) -> Vec<Line<'static>> {
    match view {
        CacheGraphView::Breakdown => vec![],
        CacheGraphView::Stats => render_stats_body(theme, metrics, width),
        CacheGraphView::PerTurn
        | CacheGraphView::CumulativePercent
        | CacheGraphView::CumulativeTotal => render_graph_body(theme, metrics, width, view),
    }
}

fn render_graph_body(
    theme: &Theme,
    metrics: &CacheSessionMetrics,
    width: u16,
    view: CacheGraphView,
) -> Vec<Line<'static>> {
    let messages = &metrics.all_messages;
    let mut lines = Vec::new();
    let label = match view {
        CacheGraphView::PerTurn => "Per-turn (%)",
        CacheGraphView::CumulativePercent => "Cumulative (aggregate) %",
        CacheGraphView::CumulativeTotal => "Cumulative (aggregate) total",
        _ => "",
    };
    lines.push(accent_line(
        theme,
        format!("Cache hit trend (whole session timeline) — {label}"),
    ));
    if metrics.estimated_count > 0 {
        lines.push(warning_line(
            theme,
            format!(
                "⚠ {}/{} turns had no provider usage — sizes estimated from content (cache hit stays 0). Common after resume with some proxies.",
                metrics.estimated_count,
                messages.len()
            ),
        ));
    }
    match view {
        CacheGraphView::PerTurn => lines.push(dim_line(
            theme,
            "Per-turn cache hit % = cacheRead / (input + cacheRead + cacheWrite)",
        )),
        CacheGraphView::CumulativePercent => lines.push(dim_line(
            theme,
            "Aggregate hit % = aggCacheRead / (aggInput + aggCacheRead + aggCacheWrite)",
        )),
        CacheGraphView::CumulativeTotal => lines.push(dim_line(
            theme,
            "Aggregate token volumes: input  ░ cacheWrite  ▒ cacheRead",
        )),
        _ => {}
    }
    lines.push(Line::from(""));
    lines.push(muted_line(
        theme,
        format_totals_line("Active branch", &metrics.active_branch_totals),
    ));
    lines.push(muted_line(
        theme,
        format_totals_line("Whole tree", &metrics.tree_totals),
    ));
    lines.push(Line::from(""));

    if messages.is_empty() {
        lines.push(warning_line(
            theme,
            "No assistant messages with usage data are available yet in this session.",
        ));
        return lines;
    }

    if view == CacheGraphView::PerTurn {
        let latest = messages.last().unwrap();
        let min = messages
            .iter()
            .map(|m| m.cache_hit_percent)
            .fold(f64::INFINITY, f64::min);
        let max = messages
            .iter()
            .map(|m| m.cache_hit_percent)
            .fold(f64::NEG_INFINITY, f64::max);
        lines.push(muted_line(
            theme,
            format!(
                "Latest: {} • Min: {} • Max: {} • Turns: {}",
                format_percent(latest.cache_hit_percent),
                format_percent(min),
                format_percent(max),
                format_int(messages.len() as u64),
            ),
        ));
    } else {
        lines.push(muted_line(
            theme,
            format!("Turns: {}", format_int(messages.len() as u64)),
        ));
    }
    lines.push(Line::from(""));

    let chart_width = (width as usize).saturating_sub(8).max(10);
    let chart_height = 10usize;
    let cum = if view != CacheGraphView::PerTurn {
        Some(compute_cumulative(messages))
    } else {
        None
    };

    match view {
        CacheGraphView::PerTurn => {
            let buckets = bucket_messages(messages, chart_width);
            let values: Vec<f64> = buckets.iter().map(|b| average_hit(b)).collect();
            lines.extend(render_bar_chart(theme, &values, chart_height));
            lines.push(dim_line(
                theme,
                format!(
                    "   1{:>width$}",
                    messages.len(),
                    width = values.len().saturating_sub(1)
                ),
            ));
            lines.push(dim_line(
                theme,
                "   assistant-message sequence in session append order",
            ));
            lines.push(Line::from(""));
            let recent_count = messages.len().min(8);
            let recent = &messages[messages.len() - recent_count..];
            lines.push(accent_line(theme, format!("Recent {recent_count} turns")));
            lines.push(dim_line(theme, "* = on current active branch"));
            for m in recent {
                let star = if m.is_on_active_branch { "*" } else { " " };
                lines.push(muted_line(
                    theme,
                    format!(
                        "#{:2}{} {:>6}  in {:>6}  cache {:>6}  {}/{}",
                        m.sequence,
                        star,
                        format_percent(m.cache_hit_percent),
                        format_int(m.input),
                        format_int(m.cache_read),
                        m.provider,
                        m.model,
                    ),
                ));
            }
        }
        CacheGraphView::CumulativePercent => {
            let cum = cum.as_ref().unwrap();
            let bucketed = bucket_max(&cum.cum_hit_percent, chart_width);
            lines.extend(render_bar_chart(theme, &bucketed, chart_height));
            lines.push(dim_line(
                theme,
                format!(
                    "   1{:>width$}",
                    messages.len(),
                    width = bucketed.len().saturating_sub(1)
                ),
            ));
            lines.push(Line::from(""));
            let recent_count = messages.len().min(8);
            let start = messages.len() - recent_count;
            lines.push(accent_line(theme, format!("Recent {recent_count} turns")));
            lines.push(dim_line(
                theme,
                "* = on current active branch  |  values are aggregate (running) totals",
            ));
            for (i, m) in messages[start..].iter().enumerate() {
                let star = if m.is_on_active_branch { "*" } else { " " };
                let hit = cum.cum_hit_percent[start + i];
                let cin = cum.cum_input[start + i];
                let cread = cum.cum_cache_read[start + i];
                lines.push(muted_line(
                    theme,
                    format!(
                        "#{:2}{} {:>6}  aggIn {:>7}  aggHit {:>7}  {}/{}",
                        m.sequence,
                        star,
                        format_percent(hit),
                        format_int(cin),
                        format_int(cread),
                        m.provider,
                        m.model,
                    ),
                ));
            }
        }
        CacheGraphView::CumulativeTotal => {
            let cum = cum.as_ref().unwrap();
            let bi = bucket_max_u64(&cum.cum_input, chart_width);
            let br = bucket_max_u64(&cum.cum_cache_read, chart_width);
            let bw = bucket_max_u64(&cum.cum_cache_write, chart_width);
            let (chart_lines, unit) = render_stacked_chart(theme, &bi, &br, &bw, chart_height);
            lines.extend(chart_lines);
            lines.push(dim_line(
                theme,
                format!(
                    "     1{:>width$}",
                    messages.len(),
                    width = bi.len().saturating_sub(1)
                ),
            ));
            lines.push(Line::from(""));
            lines.push(muted_line(
                theme,
                format!(
                    "▇ input (uncached)   ░ cache write   ▒ cache read (hit)   1 row = {} tokens",
                    format_int(unit)
                ),
            ));
            lines.push(Line::from(""));
            let recent_count = messages.len().min(8);
            let start = messages.len() - recent_count;
            lines.push(accent_line(theme, format!("Recent {recent_count} turns")));
            for (i, m) in messages[start..].iter().enumerate() {
                let star = if m.is_on_active_branch { "*" } else { " " };
                lines.push(muted_line(
                    theme,
                    format!(
                        "#{:2}{} aggIn {:>7}  aggWrite {:>7}  aggHit {:>7}  {}/{}",
                        m.sequence,
                        star,
                        format_int(cum.cum_input[start + i]),
                        format_int(cum.cum_cache_write[start + i]),
                        format_int(cum.cum_cache_read[start + i]),
                        m.provider,
                        m.model,
                    ),
                ));
            }
        }
        _ => {}
    }
    lines
}

fn render_stats_body(
    theme: &Theme,
    metrics: &CacheSessionMetrics,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(accent_line(theme, "Token/cache stats by assistant message"));
    lines.push(dim_line(
        theme,
        "B = message is on the current active branch",
    ));
    if metrics.estimated_count > 0 {
        lines.push(warning_line(
            theme,
            format!(
                "⚠ {} turns estimated from content (provider reported 0 usage / no cache metrics).",
                metrics.estimated_count
            ),
        ));
    }
    lines.push(Line::from(""));
    lines.push(accent_line(theme, "Cumulative totals"));
    lines.push(muted_line(
        theme,
        format_totals_line("Active branch", &metrics.active_branch_totals),
    ));
    lines.push(muted_line(
        theme,
        format_totals_line("Whole tree", &metrics.tree_totals),
    ));
    let tree_hit = summarize_hit_percent(&metrics.tree_totals);
    let branch_hit = summarize_hit_percent(&metrics.active_branch_totals);
    let tree_prompt = metrics
        .tree_totals
        .input
        .saturating_add(metrics.tree_totals.cache_read)
        .saturating_add(metrics.tree_totals.cache_write);
    let branch_prompt = metrics
        .active_branch_totals
        .input
        .saturating_add(metrics.active_branch_totals.cache_read)
        .saturating_add(metrics.active_branch_totals.cache_write);
    lines.push(muted_line(
        theme,
        format!(
            "Delta (tree - branch): prompt {} • received {} • cache hit {} • cache write {} • hit-rate spread {}",
            format_int(tree_prompt.saturating_sub(branch_prompt)),
            format_int(
                metrics
                    .tree_totals
                    .output
                    .saturating_sub(metrics.active_branch_totals.output)
            ),
            format_int(
                metrics
                    .tree_totals
                    .cache_read
                    .saturating_sub(metrics.active_branch_totals.cache_read)
            ),
            format_int(
                metrics
                    .tree_totals
                    .cache_write
                    .saturating_sub(metrics.active_branch_totals.cache_write)
            ),
            format_percent(tree_hit - branch_hit),
        ),
    ));
    lines.push(Line::from(""));

    if metrics.all_messages.is_empty() {
        lines.push(warning_line(
            theme,
            "No assistant messages with usage data are available yet in this session.",
        ));
        return lines;
    }

    let include_entry = width >= 92;
    let include_ts = width >= 104;
    lines.push(accent_line(theme, "Per-message breakdown"));
    let mut header = format!(
        "{:>4} {} {:24} {:>9} {:>9} {:>9} {:>9} {:>7}",
        "#", "B", "model", "prompt", "recv", "hit", "write", "hit%"
    );
    if include_entry {
        header = format!(
            "{:>4} {} {:8} {:24} {:>9} {:>9} {:>9} {:>9} {:>7}",
            "#", "B", "entry", "model", "prompt", "recv", "hit", "write", "hit%"
        );
    }
    if include_ts {
        // keep header simple when wide enough — entry already optional
    }
    lines.push(muted_line(theme, header.clone()));
    lines.push(dim_line(
        theme,
        "-".repeat(header.len().min(width as usize).max(20)),
    ));

    for m in &metrics.all_messages {
        let model = format!("{}/{}", m.provider, m.model);
        let model = if model.len() > 24 {
            format!("{}…", &model[..23.min(model.len())])
        } else {
            model
        };
        let prompt = m
            .input
            .saturating_add(m.cache_read)
            .saturating_add(m.cache_write);
        let b = if m.is_on_active_branch { "*" } else { " " };
        let row = if include_entry {
            format!(
                "{:>4} {} {:8} {:24} {:>9} {:>9} {:>9} {:>9} {:>7}",
                m.sequence,
                b,
                &m.entry_id[..m.entry_id.len().min(8)],
                model,
                format_int(prompt),
                format_int(m.output),
                format_int(m.cache_read),
                format_int(m.cache_write),
                format_percent(m.cache_hit_percent),
            )
        } else {
            format!(
                "{:>4} {} {:24} {:>9} {:>9} {:>9} {:>9} {:>7}",
                m.sequence,
                b,
                model,
                format_int(prompt),
                format_int(m.output),
                format_int(m.cache_read),
                format_int(m.cache_write),
                format_percent(m.cache_hit_percent),
            )
        };
        lines.push(muted_line(theme, row));
    }
    lines
}

/// Build CSV matching pi-cache-graph export columns (subset sufficient for Excel).
pub fn build_cache_stats_csv(metrics: &CacheSessionMetrics) -> String {
    let mut out = String::from(
        "row_type,scope,assistant_messages,sequence,is_on_active_branch,entry_id,timestamp,provider,model,prompt_tokens,received_tokens,cache_hit_tokens,cache_write_tokens,total_tokens,cache_hit_percent,notes\n",
    );
    let push_summary = |out: &mut String, scope: &str, totals: &CacheUsageTotals, notes: &str| {
        let prompt = totals
            .input
            .saturating_add(totals.cache_read)
            .saturating_add(totals.cache_write);
        // sequence,is_on,entry,timestamp,provider,model left empty for summary rows
        out.push_str(&format!(
            "summary,{scope},{},,,,,,,{},{},{},{},{},{:.4},{}\n",
            totals.assistant_messages,
            prompt,
            totals.output,
            totals.cache_read,
            totals.cache_write,
            totals.total_tokens,
            summarize_hit_percent(totals),
            csv_escape(notes),
        ));
    };
    push_summary(
        &mut out,
        "active_branch",
        &metrics.active_branch_totals,
        "active-branch cumulative totals",
    );
    push_summary(
        &mut out,
        "whole_tree",
        &metrics.tree_totals,
        "whole-tree cumulative totals",
    );
    for m in &metrics.all_messages {
        let prompt = m
            .input
            .saturating_add(m.cache_read)
            .saturating_add(m.cache_write);
        let scope = if m.is_on_active_branch {
            "active_branch"
        } else {
            "other_branch"
        };
        out.push_str(&format!(
            "message,{scope},,{},{},{},{},{},{},{},{},{},{},{},{:.4},{}\n",
            m.sequence,
            m.is_on_active_branch,
            csv_escape(&m.entry_id),
            csv_escape(&m.timestamp),
            csv_escape(&m.provider),
            csv_escape(&m.model),
            prompt,
            m.output,
            m.cache_read,
            m.cache_write,
            m.total_tokens,
            m.cache_hit_percent,
            "per-message row",
        ));
    }
    out
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub fn sanitize_export_name(name: &str) -> String {
    let sanitized: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches(|c| c == '-' || c == '.').to_string();
    if sanitized.is_empty() {
        "session".into()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_and_csv_basics() {
        let metrics = CacheSessionMetrics {
            all_messages: vec![AssistantUsageMetric {
                sequence: 1,
                active_branch_sequence: Some(1),
                entry_id: "a1".into(),
                timestamp: "2026-01-01T00:00:00Z".into(),
                provider: "xai".into(),
                model: "grok".into(),
                input: 20,
                output: 5,
                cache_read: 80,
                cache_write: 0,
                total_tokens: 105,
                cache_hit_percent: 80.0,
                is_on_active_branch: true,
                usage_estimated: false,
            }],
            active_branch_messages: vec![],
            tree_totals: CacheUsageTotals {
                input: 20,
                output: 5,
                cache_read: 80,
                cache_write: 0,
                total_tokens: 105,
                assistant_messages: 1,
            },
            active_branch_totals: CacheUsageTotals {
                input: 20,
                output: 5,
                cache_read: 80,
                cache_write: 0,
                total_tokens: 105,
                assistant_messages: 1,
            },
            estimated_count: 0,
        };
        assert!((summarize_hit_percent(&metrics.tree_totals) - 80.0).abs() < 0.01);
        let csv = build_cache_stats_csv(&metrics);
        assert!(csv.contains("message"));
        assert!(csv.contains("80"));
        assert_eq!(sanitize_export_name("My Session!"), "My-Session");
    }
}
