use serde::Deserialize;
// This is missing data!!!!! not everything is stored!!!!

#[derive(Deserialize, Debug)]
struct UltraResponse {
    #[serde(rename(deserialize = "sv_streamEntries"))]
    sv_stream_entries: Vec<SvStreamEntry>,
}

#[derive(Deserialize, Debug)]
struct SvStreamEntry {
    se_timestamp: usize,
    #[serde(rename(deserialize = "se_courseId"))]
    se_course_id: Option<String>,
    #[serde(rename(deserialize = "se_itemUri"))]
    se_item_uri: Option<String>,
    #[serde(rename(deserialize = "extraAttribs"))]
    extra_attribs: ExtraAttribs,
    #[serde(rename(deserialize = "itemSpecificData"))]
    item_specific_data: ItemSpecificData,
}

#[derive(Deserialize, Debug)]
struct ExtraAttribs {
    event_type: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ItemSpecificData {
    title: String,
    #[serde(rename(deserialize = "courseContentId"))]
    course_content_id: Option<String>,
    #[serde(rename(deserialize = "contentDetails"))]
    content_details: Option<ContentDetails>,
}

#[derive(Deserialize, Debug)]
struct ContentDetails {
    #[serde(rename(deserialize = "isFolder"))]
    is_folder: bool,
    #[serde(rename(deserialize = "isBbPage"))]
    is_bb_page: bool,
}
