// learn.uq.edu.au/learn/api/v1/stream/ultra
// has information about every file for every course in every directory!
//
// sv_streamEntries { // is a list of objects representing each file/directory/announcement/whatever over every course in every directory and subdirectory!
//  extraAttribs.event_type, // i think has something about what the item type is maybe???
//  se_timestamp,
//  se_courseId,
//  se_itemUri,
//  itemSpecificData {
//      title,
//      courseContentId,
//      contentDetails {
//          contentHandler, // maybe this matters???
//          isFolder,
//          isBbPage,
//      }
//  }
//
// process:
//
// Authenticate with blackboard
// Get list of courses
// When in a course
//  Get side bar
//      Get items under sidebar

#![allow(dead_code)]

mod ultra_data;

pub struct Course {
    display_id: String,
    display_name: String,
    // The weird id that looks something like "_172757_1"
    id: String,
    tabs: Vec<TabEntry>,
    // other stuff
}

// Sidebar entry
enum TabEntry {
    Link(String),
    Directory(Vec<FileEntry>),
}

// Content under a directory
// i have no idea how these data structures are formatted
// blackboard is such a mess
enum FileEntry {
    Directory(Vec<FileEntry>),
    File {
        title: String,
        link: String,
        attachments: Vec<()>,
        content_id: String,
        course_id: String,
        // https://learn.uq.edu.au/webapps/blackboard/execute/content/file?cmd=view&content_id={content_id}&course_id={course_id}
        // is the url of the file
    },
}
