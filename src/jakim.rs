//! Client for JAKIM's official MyeHalal directory, now served from the
//! main Halal Malaysia Portal (www.halal.gov.my) — the old
//! myehalal.halal.gov.my/portal-halal/v1/... host 301-redirects there.
//!
//! The portal is a plain server-rendered PHP site with no public JSON API,
//! so results are scraped from the same HTML the browser-based search page
//! renders. Both the search listing and the per-company detail popup accept
//! their parameters as a GET query string, which keeps the client simple.

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

const SEARCH_URL: &str = "https://www.halal.gov.my/index.php";
const DETAIL_URL: &str = "https://www.halal.gov.my/directory/slm_viewdetail.php";

/// Opaque routing token the portal expects on every directory search
/// request (base64 of `directory/index_directory;;;;`). It never changes
/// per-query, only the negeri/category/cari/page params do.
const DATA_PARAM: &str = "ZGlyZWN0b3J5L2luZGV4X2RpcmVjdG9yeTs7Ozs=";

/// (form value, display label) for the "State" dropdown, in portal order.
pub const STATES: &[(&str, &str)] = &[
    ("", "All States"),
    ("01", "Johor"),
    ("02", "Kedah"),
    ("03", "Kelantan"),
    ("04", "Melaka"),
    ("05", "Negeri Sembilan"),
    ("06", "Pahang"),
    ("07", "Pulau Pinang"),
    ("08", "Perak"),
    ("09", "Perlis"),
    ("10", "Selangor"),
    ("11", "Terengganu"),
    ("12", "Sabah"),
    ("13", "Sarawak"),
    ("14", "W.P. Kuala Lumpur"),
    ("15", "W.P. Labuan"),
    ("16", "W.P. Putrajaya"),
];

/// (form value, display label) for the "Category" dropdown, in portal order.
pub const CATEGORIES: &[(&str, &str)] = &[
    ("", "All Categories"),
    ("BG", "Barang Gunaan"),
    ("FM", "Farmaseutikal"),
    ("IN", "International"),
    ("KO", "Kosmetik dan Dandanan Diri"),
    ("MD", "Peranti Perubatan"),
    ("OEM", "Pengilangan Kontrak/OEM"),
    ("PE", "Premis Makanan"),
    ("PL", "Logistik"),
    ("PR", "Produk Makanan/Minuman"),
    ("PS", "Rumah Sembelihan"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultKind {
    Company,
    Product,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub comp_code: String,
    pub type_: String,
    pub ty: String,
    pub kind: ResultKind,
    pub name: String,
    pub address: String,
    pub brand: String,
    pub expiry_dates: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchPage {
    pub results: Vec<SearchResult>,
    pub page: u32,
    pub total_pages: u32,
    pub total_records: u32,
}

#[derive(Debug, Clone)]
pub struct Product {
    pub name: String,
    pub brand: String,
    pub expiry: String,
}

#[derive(Debug, Clone, Default)]
pub struct CompanyDetail {
    pub name: String,
    pub address: String,
    pub state: String,
    pub phone: String,
    pub fax: String,
    pub email: String,
    pub website: String,
    pub reference_no: Vec<String>,
    pub officers: Vec<String>,
    pub products: Vec<Product>,
}

pub struct JakimClient {
    http: reqwest::Client,
}

impl JakimClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) cekhalal/0.1 (+terminal halal directory lookup)")
            .timeout(Duration::from_secs(15))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http })
    }

    pub async fn search(
        &self,
        keyword: &str,
        state: &str,
        category: &str,
        page: u32,
    ) -> Result<SearchPage> {
        let page = page.max(1);
        let resp = self
            .http
            .get(SEARCH_URL)
            .query(&[
                ("data", DATA_PARAM),
                ("negeri", state),
                ("category", category),
                ("cari", keyword),
                ("page", &page.to_string()),
            ])
            .send()
            .await
            .context("request to MyeHalal directory failed")?
            .error_for_status()
            .context("MyeHalal directory returned an error status")?;
        let body = resp.text().await.context("failed to read response body")?;
        parse_search_page(&body, page)
    }

    /// Search food/drink *products* by name, across every company —
    /// distinct from `search()`, which only ever matches company names.
    ///
    /// The portal exposes this as the "Produk" tab on its results page,
    /// backed by a completely different query (`directory_halal_produk.php`)
    /// than the default company listing. It only works when `category` is
    /// pinned to `"PR"` (Produk Makanan/Minuman) *and* `ty=PR` is sent —
    /// either alone throws a server-side fatal error (confirmed against the
    /// live site), so both are hardcoded here rather than exposed as
    /// parameters.
    pub async fn search_products(&self, keyword: &str, state: &str, page: u32) -> Result<SearchPage> {
        let page = page.max(1);
        let resp = self
            .http
            .get(SEARCH_URL)
            .query(&[
                ("data", DATA_PARAM),
                ("negeri", state),
                ("category", "PR"),
                ("cari", keyword),
                ("page", &page.to_string()),
                ("ty", "PR"),
            ])
            .send()
            .await
            .context("request to MyeHalal product search failed")?
            .error_for_status()
            .context("MyeHalal product search returned an error status")?;
        let body = resp.text().await.context("failed to read response body")?;
        parse_product_search_page(&body, page)
    }

    /// Runs the company search and the product search concurrently and
    /// merges them into one page, tagged by `SearchResult::kind` so the UI
    /// can badge each row. `category` only ever filters the company side —
    /// product search is inherently food/drink-only, so it always runs
    /// regardless of which category is selected.
    pub async fn search_combined(
        &self,
        keyword: &str,
        state: &str,
        category: &str,
        page: u32,
    ) -> Result<SearchPage> {
        let (company_res, product_res) = tokio::join!(
            self.search(keyword, state, category, page),
            self.search_products(keyword, state, page),
        );
        let mut company = company_res.context("company search failed")?;
        let product = product_res.context("product search failed")?;

        let total_records = company.total_records + product.total_records;
        let total_pages = company.total_pages.max(product.total_pages);
        company.results.extend(product.results);

        Ok(SearchPage {
            results: company.results,
            page,
            total_pages,
            total_records,
        })
    }

    pub async fn detail(&self, comp_code: &str, type_: &str, ty: &str) -> Result<CompanyDetail> {
        let resp = self
            .http
            .get(DETAIL_URL)
            .query(&[("comp_code", comp_code), ("type", type_), ("ty", ty)])
            .send()
            .await
            .context("request to MyeHalal detail view failed")?
            .error_for_status()
            .context("MyeHalal detail view returned an error status")?;
        let body = resp.text().await.context("failed to read response body")?;
        parse_detail(&body)
    }
}

fn br_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<br\s*/?>").expect("static regex"))
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").expect("static regex"))
}

fn onclick_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"comp_code=([^&]+)&type=([^&']+)&ty=([^&']+)").expect("static regex")
    })
}

fn total_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"Total Record\s*:\s*(\d+)\s*-\s*Page\s*(\d+)\s*From\s*(\d+)").expect("static regex")
    })
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pull the readable text out of an element, treating `<br>` as a line
/// break and stripping every other tag (e.g. the `<font>` highlighting the
/// matched keyword). Empty lines are dropped.
fn extract_lines(el: ElementRef) -> Vec<String> {
    let html = el.inner_html();
    let with_breaks = br_re().replace_all(&html, "\n");
    let no_tags = tag_re().replace_all(&with_breaks, "");
    let decoded = decode_entities(&no_tags);
    decoded
        .lines()
        .map(normalize_ws)
        .filter(|l| !l.is_empty())
        .collect()
}

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("static selector")
}

/// Parses the "Total Record : N - Page P From T" footer common to both the
/// company and product search result pages.
fn parse_pagination(doc: &Html, requested_page: u32, result_count: usize) -> (u32, u32, u32) {
    let full_text = doc.root_element().text().collect::<Vec<_>>().join(" ");
    match total_re().captures(&full_text) {
        Some(c) => (
            c[1].parse().unwrap_or(result_count as u32),
            c[2].parse().unwrap_or(requested_page),
            c[3].parse().unwrap_or(1).max(1),
        ),
        None => (
            result_count as u32,
            requested_page,
            if result_count == 0 { 0 } else { 1 },
        ),
    }
}

fn parse_search_page(html: &str, requested_page: u32) -> Result<SearchPage> {
    let doc = Html::parse_document(html);
    let row_sel = sel("tr[onclick]");
    let td_sel = sel("td");
    let name_sel = sel("span.company-name");
    let addr_sel = sel("span.company-address");
    let brand_sel = sel("span.company-brand");

    let mut results = Vec::new();
    for row in doc.select(&row_sel) {
        let onclick = row.value().attr("onclick").unwrap_or_default();
        let Some(caps) = onclick_re().captures(onclick) else {
            continue;
        };
        let comp_code = caps[1].to_string();
        let type_ = caps[2].to_string();
        let ty = caps[3].to_string();

        let tds: Vec<_> = row.select(&td_sel).collect();
        if tds.len() < 3 {
            continue;
        }

        let name = tds[1]
            .select(&name_sel)
            .next()
            .map(extract_lines)
            .unwrap_or_default()
            .join(" ");
        let address = tds[1]
            .select(&addr_sel)
            .next()
            .map(extract_lines)
            .unwrap_or_default()
            .join(", ");
        let brand = tds[1]
            .select(&brand_sel)
            .next()
            .map(extract_lines)
            .unwrap_or_default()
            .join(", ");
        let expiry_dates = extract_lines(tds[2]);

        if name.is_empty() {
            continue;
        }

        results.push(SearchResult {
            comp_code,
            type_,
            ty,
            kind: ResultKind::Company,
            name,
            address,
            brand,
            expiry_dates,
        });
    }

    let (total_records, page, total_pages) = parse_pagination(&doc, requested_page, results.len());

    Ok(SearchPage {
        results,
        page,
        total_pages,
        total_records,
    })
}

fn product_onclick_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"comp_code=([^&']+)&type=([^&']+)").expect("static regex"))
}

/// Product-tab rows use the same `company-name`/`company-brand`/
/// `company-address` CSS classes as the company table, but they hold
/// different data here: product name, "JENAMA: <brand>", and the owning
/// company's name respectively. Reused into `SearchResult` (name = product,
/// address = company, a single-element expiry list) so the rest of the app
/// — list rendering, selection, preview fetch — needs no product-specific
/// code path.
fn parse_product_search_page(html: &str, requested_page: u32) -> Result<SearchPage> {
    let doc = Html::parse_document(html);
    let row_sel = sel("tr[onclick]");
    let td_sel = sel("td");
    let name_sel = sel("span.company-name");
    let addr_sel = sel("span.company-address");
    let brand_sel = sel("span.company-brand");

    let mut results = Vec::new();
    for row in doc.select(&row_sel) {
        let onclick = row.value().attr("onclick").unwrap_or_default();
        let Some(caps) = product_onclick_re().captures(onclick) else {
            continue;
        };
        let comp_code = caps[1].to_string();
        let type_ = caps[2].to_string();

        let tds: Vec<_> = row.select(&td_sel).collect();
        if tds.len() < 3 {
            continue;
        }

        let product_name = tds[1]
            .select(&name_sel)
            .next()
            .map(extract_lines)
            .unwrap_or_default()
            .join(" ");
        let company_name = tds[1]
            .select(&addr_sel)
            .next()
            .map(extract_lines)
            .unwrap_or_default()
            .join(", ");
        let brand = tds[1]
            .select(&brand_sel)
            .next()
            .map(extract_lines)
            .unwrap_or_default()
            .join(", ")
            .trim_start_matches("JENAMA:")
            .trim()
            .to_string();
        let expiry_dates = extract_lines(tds[2]);

        if product_name.is_empty() {
            continue;
        }

        results.push(SearchResult {
            comp_code,
            type_,
            ty: String::new(),
            kind: ResultKind::Product,
            name: product_name,
            address: company_name,
            brand,
            expiry_dates,
        });
    }

    let (total_records, page, total_pages) = parse_pagination(&doc, requested_page, results.len());

    Ok(SearchPage {
        results,
        page,
        total_pages,
        total_records,
    })
}

fn parse_detail(html: &str) -> Result<CompanyDetail> {
    let doc = Html::parse_document(html);
    let tr_sel = sel("tr");
    let td_sel = sel("td");

    let mut detail = CompanyDetail::default();

    for row in doc.select(&tr_sel) {
        let tds: Vec<_> = row.select(&td_sel).collect();
        if tds.len() != 2 {
            continue;
        }
        let label_raw = normalize_ws(&tds[0].text().collect::<String>());
        if label_raw.is_empty() {
            continue;
        }
        let label = label_raw.trim_end_matches(':').trim().to_lowercase();
        let value_lines = extract_lines(tds[1]);

        if label == "name" {
            detail.name = value_lines.join(" ");
        } else if label.contains("address") {
            detail.address = value_lines.join(", ");
        } else if label == "state" {
            detail.state = value_lines.join(" ");
        } else if label.contains("phone") {
            detail.phone = value_lines.join(" ");
        } else if label.contains("fax") {
            detail.fax = value_lines.join(" ");
        } else if label.contains("mail") {
            detail.email = value_lines.join(" ");
        } else if label.contains("website") {
            detail.website = value_lines.join(" ");
        } else if label.contains("reference") {
            detail.reference_no = value_lines;
        } else if label.contains("officer") {
            detail.officers = value_lines;
        }
    }

    let product_row_sel = sel("table[border=\"1\"] tr");
    let mut products = Vec::new();
    for row in doc.select(&product_row_sel) {
        let tds: Vec<_> = row.select(&td_sel).collect();
        if tds.len() != 4 {
            continue;
        }
        if tds[1].value().attr("class") != Some("txt") {
            continue;
        }
        let name = extract_lines(tds[1]).join(" ");
        let brand = extract_lines(tds[2]).join(" ");
        let expiry = extract_lines(tds[3]).join(" ");
        if name.is_empty() {
            continue;
        }
        products.push(Product { name, brand, expiry });
    }
    detail.products = products;

    Ok(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_results_page() {
        let html = include_str!("../tests/fixtures/jakim_search.html");
        let page = parse_search_page(html, 1).expect("should parse");

        assert_eq!(page.total_records, 11);
        assert_eq!(page.page, 1);
        assert_eq!(page.total_pages, 1);
        assert_eq!(page.results.len(), 11);

        let first = &page.results[0];
        assert_eq!(first.comp_code, "COMP-20110825-101221");
        assert_eq!(first.type_, "C");
        assert_eq!(first.ty, "CO");
        assert_eq!(first.kind, ResultKind::Company);
        assert!(first.name.contains("CEREAL PARTNERS"));
        assert!(first.name.contains("NESTLE"));
        assert!(first.address.contains("PETALING JAYA"));
        assert_eq!(first.expiry_dates.len(), 6);
        assert_eq!(first.expiry_dates[0], "29/02/2028");
    }

    #[test]
    fn parses_product_search_results_page() {
        let html = include_str!("../tests/fixtures/jakim_product_search.html");
        let page = parse_product_search_page(html, 1).expect("should parse");

        assert_eq!(page.total_records, 172);
        assert_eq!(page.page, 1);
        assert_eq!(page.total_pages, 9);
        assert!(!page.results.is_empty());

        let first = &page.results[0];
        assert_eq!(first.type_, "C");
        assert!(first.ty.is_empty());
        assert_eq!(first.kind, ResultKind::Product);
        assert!(first.name.contains("NESTLE"));
        assert!(first.name.contains("MILO"));
        assert_eq!(first.brand, "NESTLE/MILO");
        assert!(first.address.contains("NESTLE MANUFACTURING"));
        assert_eq!(first.expiry_dates.len(), 1);
    }

    #[test]
    fn parses_company_detail_page() {
        let html = include_str!("../tests/fixtures/jakim_detail.html");
        let detail = parse_detail(html).expect("should parse");

        assert_eq!(detail.name, "NESTLE PRODUCTS SDN. BHD.");
        assert!(detail.address.contains("PETALING JAYA"));
        assert_eq!(detail.state, "Selangor");
        assert_eq!(detail.phone, "0133138492");
        assert_eq!(detail.email, "sitihazlin.jantan@my.nestle.com");
        assert_eq!(detail.website, "www.nestle.com.my");
        assert_eq!(detail.reference_no.len(), 2);
        assert!(detail.officers.iter().any(|o| o.contains("ROSLIN")));
        assert_eq!(detail.products.len(), 2);
        assert_eq!(detail.products[0].brand, "NESTLE HEALTH SCIENCE");
        assert_eq!(detail.products[1].expiry, "15/06/2027");
    }
}
