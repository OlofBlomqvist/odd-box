use ratatui::widgets::ScrollbarState;


#[derive(Debug,Default)]
pub struct ScrollStateWrapper {
    pub vertical_scroll_state: ScrollbarState,
    pub _horizontal_scroll_state: ScrollbarState,
    pub vertical_scroll: Option<usize>,
    pub _horizontal_scroll: Option<usize>,    
    pub total_rows : usize,
    pub visible_rows : usize,
    pub area_height : usize,
    pub area_width : usize,
    pub scroll_bar_hovered : bool,
    pub last_mouse_down_y_pos : usize
}

impl ScrollStateWrapper {

    pub fn scroll_up(&mut self, count:Option<usize>) {
        match self.vertical_scroll {
            Some(current) if current > 0 => {
                let new_val = current.saturating_sub(count.unwrap_or(1)).max(0);
                self.vertical_scroll = Some(new_val);
                self.vertical_scroll_state = self.vertical_scroll_state.position(new_val);
            }
            None => {
                let max = self.total_rows.saturating_sub(self.area_height);
                let new_val = max.saturating_sub(count.unwrap_or(1));
                self.vertical_scroll = Some(new_val);
                self.vertical_scroll_state = self.vertical_scroll_state.position(new_val);
            }
            _ => {}
        }
    }

    pub fn calculate_thumb_size(&self) -> f32 {
        if self.total_rows <= self.area_height {
            // this is just if we dont need a scrollbar - in which case its just going to be hidden anyway
            self.area_height as f32
        } else {
            let thumb_size = (self.area_height as f64 / self.total_rows as f64) * self.area_height as f64;
            thumb_size.ceil() as f32
        }
    }
    pub fn handle_mouse_move(&mut self, column: u16, row: u16) {
        let thumb_size = self.calculate_thumb_size().max(1.0);
        let max_scroll = self.total_rows.saturating_sub(self.area_height);
        let vscroll = self.vertical_scroll.unwrap_or(max_scroll);
        let thumb_position = if self.total_rows > self.area_height {
            (vscroll as f32 / (self.total_rows as f32 - self.area_height as f32)) * (self.area_height as f32 - thumb_size)
        } else {
            1.0
        }.max(1.0);
        let horizontal_match = column as usize >= self.area_width.saturating_sub(1)  && column as usize <= self.area_width.saturating_add(1);
        let vertical_match = (row as isize >= thumb_position as isize - 2) && row as usize <= (thumb_position + thumb_size + 1.0) as usize;
        //self.dbg = format!("dragging pos: {row}/{column} - vscroll: {} - tpos: {thumb_position}  | V: {vertical_match}, H: {horizontal_match}",vscroll);
        self.scroll_bar_hovered = horizontal_match && vertical_match;
    }


    pub fn handle_mouse_drag(&mut self, _column: u16, row: u16) {

        if self.scroll_bar_hovered {

            let max_scroll = self.total_rows.saturating_sub(self.area_height);
            
            let click_position = (row as usize).min(self.area_height).max(0);
            let percentage = click_position as f32 / self.area_height as f32;
            let scroll_to = (percentage * self.total_rows as f32).round() as usize;

            let new_val = scroll_to.min(max_scroll);

            if new_val == max_scroll {
                self.vertical_scroll = None;
                self.vertical_scroll_state = self.vertical_scroll_state.position(new_val);    
            } else {
                self.vertical_scroll = Some(new_val);
                self.vertical_scroll_state = self.vertical_scroll_state.position(new_val);
            }

        } else {

            self.last_mouse_down_y_pos = row as usize;
        }
        

    }

    pub fn scroll_down(&mut self, count: Option<usize>) {

        let max = self.total_rows.saturating_sub(self.area_height);
        let current = self.vertical_scroll.unwrap_or(max);

        if current < max {
            let new_val = current.saturating_add(count.unwrap_or(1)).min(max);
            self.vertical_scroll = Some(new_val);
            self.vertical_scroll_state = self.vertical_scroll_state.position(new_val);
        }
        else {
            self.vertical_scroll = None;
        }
    }
    
}
