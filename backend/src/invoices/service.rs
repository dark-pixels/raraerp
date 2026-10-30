use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point, Rgb,
};
use rust_decimal::{prelude::ToPrimitive, Decimal};

use crate::errors::{AppError, AppResult};
use crate::common::format_inr;
use crate::settings::CompanySettings;

pub struct InvoiceLine {
    pub name: String,
    pub hsn: Option<String>,
    pub qty: Decimal,
    pub rate: Decimal,
    pub discount: Decimal,
    pub taxable: Decimal,
    pub gst_rate: Decimal,
    pub tax: Decimal,
    pub total: Decimal,
}

pub struct InvoiceDoc {
    pub company: CompanySettings,
    pub invoice_number: String,
    pub invoice_date: chrono::NaiveDate,
    pub due_date: Option<chrono::NaiveDate>,
    pub status: String,
    pub payment_method: Option<String>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_gstin: Option<String>,
    pub billing_address: Option<String>,
    pub subtotal: Decimal,
    pub discount_amount: Decimal,
    pub taxable_value: Decimal,
    pub cgst: Decimal,
    pub sgst: Decimal,
    pub igst: Decimal,
    pub grand_total: Decimal,
    pub amount_paid: Decimal,
    pub outstanding: Decimal,
    pub notes: Option<String>,
    pub lines: Vec<InvoiceLine>,
}

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 15.0;
const RIGHT: f32 = PAGE_W - MARGIN;
/// Approximate Helvetica advance width (em) used to right-align text.
const AVG_CHAR_EM: f32 = 0.52;
const PT_TO_MM: f32 = 0.3528;

/// The built-in PDF fonts only cover WinAnsi, so non-Latin characters
/// (including the rupee sign) are replaced to avoid garbled output.
fn pdf_safe(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '₹' => out.push_str("Rs."),
            c if (c as u32) < 0x80 || ('\u{a0}'..='\u{ff}').contains(&c) => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    let s = pdf_safe(s);
    if s.chars().count() <= max {
        s
    } else {
        format!("{}...", s.chars().take(max.saturating_sub(3)).collect::<String>())
    }
}

fn words_below_thousand(n: u64) -> String {
    const ONES: [&str; 20] = [
        "", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
        "Eleven", "Twelve", "Thirteen", "Fourteen", "Fifteen", "Sixteen", "Seventeen", "Eighteen", "Nineteen",
    ];
    const TENS: [&str; 10] = ["", "", "Twenty", "Thirty", "Forty", "Fifty", "Sixty", "Seventy", "Eighty", "Ninety"];
    let mut parts = Vec::new();
    let hundreds = n / 100;
    let rest = n % 100;
    if hundreds > 0 {
        parts.push(format!("{} Hundred", ONES[hundreds as usize]));
    }
    if rest > 0 {
        if rest < 20 {
            parts.push(ONES[rest as usize].to_string());
        } else {
            let t = TENS[(rest / 10) as usize];
            let o = ONES[(rest % 10) as usize];
            parts.push(if o.is_empty() { t.to_string() } else { format!("{} {}", t, o) });
        }
    }
    parts.join(" ")
}

/// Converts a whole number to words using the Indian system (lakh, crore).
fn number_to_words(mut n: u64) -> String {
    if n == 0 {
        return "Zero".into();
    }
    let mut parts = Vec::new();
    let crore = n / 10_000_000;
    n %= 10_000_000;
    let lakh = n / 100_000;
    n %= 100_000;
    let thousand = n / 1000;
    n %= 1000;
    if crore > 0 {
        parts.push(format!("{} Crore", number_to_words(crore)));
    }
    if lakh > 0 {
        parts.push(format!("{} Lakh", words_below_thousand(lakh)));
    }
    if thousand > 0 {
        parts.push(format!("{} Thousand", words_below_thousand(thousand)));
    }
    if n > 0 {
        parts.push(words_below_thousand(n));
    }
    parts.join(" ")
}

pub fn amount_in_words(amount: Decimal) -> String {
    let amount = amount.round_dp(2).abs();
    let rupees = amount.trunc().to_u64().unwrap_or(0);
    let paise = ((amount - amount.trunc()) * Decimal::from(100)).round().to_u64().unwrap_or(0);
    let mut s = format!("Rupees {}", number_to_words(rupees));
    if paise > 0 {
        s.push_str(&format!(" and {} Paise", number_to_words(paise)));
    }
    s.push_str(" Only");
    s
}

struct Writer {
    layer: PdfLayerReference,
    regular: IndirectFontRef,
    bold: IndirectFontRef,
}

impl Writer {
    fn text(&self, s: &str, size: f32, x: f32, y: f32, bold: bool) {
        let font = if bold { &self.bold } else { &self.regular };
        self.layer.use_text(pdf_safe(s), size, Mm(x), Mm(y), font);
    }

    fn text_right(&self, s: &str, size: f32, right: f32, y: f32, bold: bool) {
        let width = s.chars().count() as f32 * size * AVG_CHAR_EM * PT_TO_MM;
        self.text(s, size, right - width, y, bold);
    }

    fn hline(&self, y: f32, thickness: f32) {
        self.layer.set_outline_thickness(thickness);
        self.layer.add_line(Line {
            points: vec![(Point::new(Mm(MARGIN), Mm(y)), false), (Point::new(Mm(RIGHT), Mm(y)), false)],
            is_closed: false,
        });
    }
}

// Column x positions (left edges, or right edges for numeric columns).
const C_NO: f32 = MARGIN;
const C_ITEM: f32 = MARGIN + 8.0;
const C_HSN: f32 = 92.0;
const C_QTY_R: f32 = 120.0;
const C_RATE_R: f32 = 140.0;
const C_GST_R: f32 = 152.0;
const C_TAX_R: f32 = 172.0;
const C_TOTAL_R: f32 = RIGHT;

fn table_header(w: &Writer, y: f32) -> f32 {
    w.hline(y + 4.5, 0.6);
    w.text("#", 8.5, C_NO, y, true);
    w.text("Item", 8.5, C_ITEM, y, true);
    w.text("HSN", 8.5, C_HSN, y, true);
    w.text_right("Qty", 8.5, C_QTY_R, y, true);
    w.text_right("Rate", 8.5, C_RATE_R, y, true);
    w.text_right("GST", 8.5, C_GST_R, y, true);
    w.text_right("Tax", 8.5, C_TAX_R, y, true);
    w.text_right("Amount", 8.5, C_TOTAL_R, y, true);
    w.hline(y - 2.0, 0.4);
    y - 7.0
}

pub fn generate_invoice_pdf(doc: &InvoiceDoc) -> AppResult<Vec<u8>> {
    let (pdf, page1, layer1) = PdfDocument::new(
        format!("Invoice {}", doc.invoice_number),
        Mm(PAGE_W),
        Mm(PAGE_H),
        "Invoice",
    );
    let regular = pdf
        .add_builtin_font(BuiltinFont::Helvetica)
        .map_err(|e| AppError::Internal(format!("PDF font error: {}", e)))?;
    let bold = pdf
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .map_err(|e| AppError::Internal(format!("PDF font error: {}", e)))?;

    let mut w = Writer { layer: pdf.get_page(page1).get_layer(layer1), regular, bold };
    w.layer.set_outline_color(Color::Rgb(Rgb::new(0.6, 0.6, 0.6, None)));

    let c = &doc.company;
    let mut y = PAGE_H - MARGIN - 4.0;

    // ── Seller block ─────────────────────────────────────────────
    w.text(&c.name, 18.0, MARGIN, y, true);
    w.text_right("TAX INVOICE", 14.0, RIGHT, y, true);
    y -= 6.0;
    if let Some(addr) = &c.address {
        for line in addr.lines().take(3) {
            w.text(&truncate(line, 70), 9.0, MARGIN, y, false);
            y -= 4.2;
        }
    }
    let contact: Vec<String> = [c.phone.as_ref().map(|p| format!("Ph: {}", p)), c.email.clone()]
        .into_iter()
        .flatten()
        .collect();
    if !contact.is_empty() {
        w.text(&contact.join("  |  "), 9.0, MARGIN, y, false);
        y -= 4.2;
    }
    if let Some(g) = &c.gstin {
        w.text(&format!("GSTIN: {}", g), 9.0, MARGIN, y, true);
        y -= 4.2;
    }

    // ── Invoice meta (right column) ──────────────────────────────
    let mut my = PAGE_H - MARGIN - 10.0;
    w.text_right(&format!("Invoice No: {}", doc.invoice_number), 9.5, RIGHT, my, true);
    my -= 4.5;
    w.text_right(&format!("Date: {}", doc.invoice_date.format("%d-%m-%Y")), 9.0, RIGHT, my, false);
    my -= 4.5;
    if let Some(due) = doc.due_date {
        w.text_right(&format!("Due: {}", due.format("%d-%m-%Y")), 9.0, RIGHT, my, false);
        my -= 4.5;
    }
    w.text_right(&format!("Status: {}", doc.status.replace('_', " ").to_uppercase()), 9.0, RIGHT, my, false);
    my -= 4.5;

    y = y.min(my) - 3.0;
    w.hline(y, 0.6);
    y -= 6.0;

    // ── Buyer block ──────────────────────────────────────────────
    w.text("Bill To", 9.0, MARGIN, y, true);
    y -= 5.0;
    w.text(doc.customer_name.as_deref().unwrap_or("Walk-in Customer"), 10.5, MARGIN, y, true);
    y -= 4.5;
    if let Some(addr) = &doc.billing_address {
        for line in addr.lines().take(3) {
            w.text(&truncate(line, 80), 9.0, MARGIN, y, false);
            y -= 4.2;
        }
    }
    if let Some(p) = &doc.customer_phone {
        if !p.is_empty() {
            w.text(&format!("Ph: {}", p), 9.0, MARGIN, y, false);
            y -= 4.2;
        }
    }
    if let Some(g) = &doc.customer_gstin {
        w.text(&format!("GSTIN: {}", g), 9.0, MARGIN, y, false);
        y -= 4.2;
    }
    y -= 4.0;

    // ── Line items ───────────────────────────────────────────────
    y = table_header(&w, y);
    for (i, l) in doc.lines.iter().enumerate() {
        if y < 60.0 {
            let (page, layer) = pdf.add_page(Mm(PAGE_W), Mm(PAGE_H), "Invoice");
            w.layer = pdf.get_page(page).get_layer(layer);
            w.layer.set_outline_color(Color::Rgb(Rgb::new(0.6, 0.6, 0.6, None)));
            y = PAGE_H - MARGIN - 4.0;
            w.text(&format!("{} (continued)", doc.invoice_number), 9.0, MARGIN, y, true);
            y = table_header(&w, y - 8.0);
        }
        w.text(&(i + 1).to_string(), 8.5, C_NO, y, false);
        let mut name = truncate(&l.name, 42);
        if l.discount > Decimal::ZERO {
            name = truncate(&format!("{} (-{}%)", l.name, l.discount.normalize()), 42);
        }
        w.text(&name, 8.5, C_ITEM, y, false);
        w.text(l.hsn.as_deref().unwrap_or("-"), 8.5, C_HSN, y, false);
        w.text_right(&l.qty.normalize().to_string(), 8.5, C_QTY_R, y, false);
        w.text_right(&format_inr(l.rate), 8.5, C_RATE_R, y, false);
        w.text_right(&format!("{}%", l.gst_rate.normalize()), 8.5, C_GST_R, y, false);
        w.text_right(&format_inr(l.tax), 8.5, C_TAX_R, y, false);
        w.text_right(&format_inr(l.total), 8.5, C_TOTAL_R, y, false);
        let _ = l.taxable;
        y -= 5.5;
    }
    w.hline(y + 2.0, 0.4);
    y -= 4.0;

    // ── Totals ───────────────────────────────────────────────────
    let label_x = 125.0;
    let total_row = |label: &str, value: Decimal, bold: bool, y: &mut f32| {
        w.text(label, 9.0, label_x, *y, bold);
        w.text_right(&format_inr(value), 9.0, RIGHT, *y, bold);
        *y -= 5.0;
    };
    total_row("Subtotal", doc.subtotal, false, &mut y);
    if doc.discount_amount > Decimal::ZERO {
        total_row("Discount", -doc.discount_amount, false, &mut y);
    }
    total_row("Taxable Value", doc.taxable_value, false, &mut y);
    if doc.igst > Decimal::ZERO {
        total_row("IGST", doc.igst, false, &mut y);
    } else {
        total_row("CGST", doc.cgst, false, &mut y);
        total_row("SGST", doc.sgst, false, &mut y);
    }
    w.hline(y + 3.0, 0.4);
    y -= 1.0;
    w.text("Grand Total (Rs.)", 11.0, label_x, y, true);
    w.text_right(&format_inr(doc.grand_total), 11.0, RIGHT, y, true);
    y -= 6.0;
    w.text("Paid", 9.0, label_x, y, false);
    w.text_right(&format_inr(doc.amount_paid), 9.0, RIGHT, y, false);
    y -= 5.0;
    if doc.outstanding > Decimal::ZERO {
        w.text("Balance Due", 9.0, label_x, y, true);
        w.text_right(&format_inr(doc.outstanding), 9.0, RIGHT, y, true);
        y -= 5.0;
    }

    y -= 3.0;
    w.text("Amount in words:", 9.0, MARGIN, y, true);
    y -= 4.5;
    w.text(&truncate(&amount_in_words(doc.grand_total), 110), 9.0, MARGIN, y, false);
    y -= 7.0;

    if let Some(method) = &doc.payment_method {
        w.text(&format!("Payment mode: {}", method.replace('_', " ")), 9.0, MARGIN, y, false);
        y -= 4.5;
    }
    if let Some(upi) = &c.upi_id {
        if doc.outstanding > Decimal::ZERO {
            w.text(&format!("Pay via UPI: {}", upi), 9.0, MARGIN, y, false);
            y -= 4.5;
        }
    }
    if let Some(notes) = &doc.notes {
        for line in notes.lines().take(4) {
            w.text(&truncate(line, 110), 8.5, MARGIN, y, false);
            y -= 4.2;
        }
    }

    // ── Footer ───────────────────────────────────────────────────
    w.hline(32.0, 0.4);
    w.text(&format!("For {}", c.name), 9.0, 140.0, 26.0, true);
    w.text("Authorised Signatory", 8.5, 140.0, 14.0, false);
    w.text("Thank you for your business!", 9.0, MARGIN, 26.0, false);
    w.text("This is a computer-generated invoice.", 7.5, MARGIN, 14.0, false);

    pdf.save_to_bytes().map_err(|e| AppError::Internal(format!("PDF generation failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inr_grouping() {
        assert_eq!(format_inr("1234567.891".parse().unwrap()), "12,34,567.89");
        assert_eq!(format_inr("999".parse().unwrap()), "999.00");
        assert_eq!(format_inr("1000".parse().unwrap()), "1,000.00");
        assert_eq!(format_inr("100000".parse().unwrap()), "1,00,000.00");
        assert_eq!(format_inr("-2500.5".parse().unwrap()), "-2,500.50");
    }

    #[test]
    fn words() {
        assert_eq!(amount_in_words("0".parse().unwrap()), "Rupees Zero Only");
        assert_eq!(amount_in_words("115".parse().unwrap()), "Rupees One Hundred Fifteen Only");
        assert_eq!(
            amount_in_words("1250050.75".parse().unwrap()),
            "Rupees Twelve Lakh Fifty Thousand Fifty and Seventy Five Paise Only"
        );
        assert_eq!(amount_in_words("23000000".parse().unwrap()), "Rupees Two Crore Thirty Lakh Only");
    }

    #[test]
    fn renders_multi_page_invoice() {
        let line = |i: u32| InvoiceLine {
            name: format!("Soft-close hinge 35mm ₹ model {}", i),
            hsn: Some("8302".into()),
            qty: Decimal::from(i),
            rate: "125.50".parse().unwrap(),
            discount: if i % 3 == 0 { Decimal::from(5) } else { Decimal::ZERO },
            taxable: Decimal::from(100 * i),
            gst_rate: Decimal::from(18),
            tax: Decimal::from(18 * i),
            total: Decimal::from(118 * i),
        };
        let doc = InvoiceDoc {
            company: CompanySettings {
                name: "RARA Enterprise".into(),
                gstin: Some("27AAPFU0939F1ZV".into()),
                address: Some("12 Market Road\nPune 411001".into()),
                phone: Some("9800000000".into()),
                email: None,
                logo_url: None,
                invoice_prefix: "INV".into(),
                upi_id: Some("shop@upi".into()),
                upi_name: None,
            },
            invoice_number: "INV-2026-000001".into(),
            invoice_date: chrono::NaiveDate::from_ymd_opt(2026, 10, 29).unwrap(),
            due_date: chrono::NaiveDate::from_ymd_opt(2026, 11, 28),
            status: "partially_paid".into(),
            payment_method: Some("upi".into()),
            customer_name: Some("Shah Interiors".into()),
            customer_phone: Some("9811111111".into()),
            customer_gstin: Some("27ABCDE1234F1Z5".into()),
            billing_address: Some("Line 1\nLine 2".into()),
            subtotal: Decimal::from(50_000),
            discount_amount: Decimal::from(500),
            taxable_value: Decimal::from(49_500),
            cgst: Decimal::from(4455),
            sgst: Decimal::from(4455),
            igst: Decimal::ZERO,
            grand_total: Decimal::from(58_410),
            amount_paid: Decimal::from(10_000),
            outstanding: Decimal::from(48_410),
            notes: Some("Deliver to site".into()),
            lines: (1..=60).map(line).collect(),
        };
        let bytes = generate_invoice_pdf(&doc).expect("pdf renders");
        if let Ok(path) = std::env::var("INVOICE_PDF_PREVIEW") {
            std::fs::write(path, &bytes).unwrap();
        }
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 2000);
    }

    #[test]
    fn pdf_safe_replaces_unsupported_chars() {
        assert_eq!(pdf_safe("₹ 100"), "Rs. 100");
        assert_eq!(pdf_safe("MR SHAH"), "MR SHAH");
        assert_eq!(pdf_safe("Café"), "Café");
        assert_eq!(pdf_safe("नमस्ते"), "??????");
    }
}
