pub fn wrapper() {
    let svg_data = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
        icondata::LuSend.data
    );
    let handle = iced::widget::svg::Handle::from_memory(svg_data.into_bytes());
}
