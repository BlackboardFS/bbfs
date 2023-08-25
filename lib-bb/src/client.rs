use crate::{Course, CourseItem, CourseItemType};

pub trait BBClient {
    fn get_courses(&self) -> Vec<Course>;
    fn get_course_contents(&self, course: &Course) -> Vec<CourseItem>;

    fn get_item_size(&self, item: &CourseItem) -> usize;
    fn get_item_contents(&self, item: &CourseItem) -> Vec<u8>;
}

pub struct BBMockClient;

impl BBClient for BBMockClient {
    fn get_courses(&self) -> Vec<Course> {
        vec![
            Course {
                short_name: "MATH2401".into(),
                full_name: "[MATH2401] Real analysis".into(),
                id: "".into(),
            },
            Course {
                short_name: "MATH1072".into(),
                full_name: "[MATH1072] Multivariate calculus".into(),
                id: "".into(),
            },
            Course {
                short_name: "STAT1301".into(),
                full_name: "[STAT1301] Advanced analysis of scientific data".into(),
                id: "".into(),
            },
            Course {
                short_name: "COMP3506".into(),
                full_name: "[COMP3506] Algorithms & data structures".into(),
                id: "".into(),
            },
        ]
    }

    fn get_course_contents(&self, course: &Course) -> Vec<CourseItem> {
        if course.short_name == "MATH2401" {
            vec![CourseItem {
                name: "Assignment 1".into(),
                url: "https://learn.uq.edu.au/bbcswebdav/pid-9222876-dt-content-rid-56218459_1/xid-56218459_1".into(),
                ty: CourseItemType::File,
            }]
        } else {
            vec![]
        }
    }

    fn get_item_size(&self, _item: &CourseItem) -> usize {
        10
    }

    fn get_item_contents(&self, _item: &CourseItem) -> Vec<u8> {
        "hellohello".into()
    }
}

// pub struct BBAPIClient;
