use rust_decimal::Decimal;
use crate::errors::AppResult;

pub async fn generate_invoice_pdf(
    invoice_number: &str,
    customer_name: Option<&str>,
    grand_total: Decimal,
) -> AppResult<Vec<u8>> {
    use printpdf::*;
    use std::io::BufWriter;

    let (doc, page1, layer1) = PdfDocument::new(
        format!("Invoice {}", invoice_number),
        Mm(210.0),
        Mm(297.0),
        "Layer 1",
    );

    let current_layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap();
    let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();

    // Header
    current_layer.use_text("RARA Enterprise ERP", 24.0, Mm(20.0), Mm(270.0), &font);
    current_layer.use_text("Furniture Hardware Shop", 12.0, Mm(20.0), Mm(262.0), &font_regular);

    // Invoice number
    current_layer.use_text(&format!("Invoice: {}", invoice_number), 16.0, Mm(20.0), Mm(245.0), &font);

    // Customer
    let cname = customer_name.unwrap_or("Walk-in Customer");
    current_layer.use_text(&format!("Customer: {}", cname), 12.0, Mm(20.0), Mm(235.0), &font_regular);

    // Total
    current_layer.use_text(&format!("Grand Total: ₹{}", grand_total), 14.0, Mm(20.0), Mm(220.0), &font);

    // Footer
    current_layer.use_text("Thank you for your business!", 10.0, Mm(20.0), Mm(20.0), &font_regular);

    let mut bytes: Vec<u8> = Vec::new();
    doc.save(&mut BufWriter::new(std::io::Cursor::new(&mut bytes))).unwrap();

    Ok(bytes)
}
