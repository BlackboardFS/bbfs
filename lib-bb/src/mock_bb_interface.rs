pub struct MockBbInterface;

impl MockBbInterface {
    pub fn get_courses(&self) /* -> ??? */ {
        todo!();
    }

    pub fn get_course_by_name(&self, _name: String) /* -> ??? */ {
        todo!();
    }

    pub fn get_course_contents(&self, _course: ()) /* -> ??? */ {
        todo!();
    }

    pub fn get_item_size(&self, _url: String) -> usize {
        todo!();
    }

    pub fn get_item_contents(&self, _url: String) /* -> ??? */ {
        todo!();
    }
}
