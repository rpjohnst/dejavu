use gml::symbol::Symbol;

#[derive(Default)]
pub struct State {
    pub score: f32,
    pub lives: f32,
    pub health: f32,

    pub show_score: bool,
    pub show_lives: bool,
    pub show_health: bool,

    pub caption_score: Symbol,
    pub caption_lives: Symbol,
    pub caption_health: Symbol,
}

#[gml::bind]
impl State {
    #[gml::get(score)]
    pub fn get_score(&self) -> f32 { self.score }
    #[gml::set(score)]
    pub fn set_score(&mut self, value: f32) { self.score = value; }

    #[gml::get(lives)]
    pub fn get_lives(&self) -> f32 { self.lives }
    #[gml::set(lives)]
    pub fn set_lives(&mut self, value: f32) { self.lives = value; }

    #[gml::get(health)]
    pub fn get_health(&self) -> f32 { self.health }
    #[gml::set(health)]
    pub fn set_health(&mut self, value: f32) { self.health = value; }

    #[gml::get(show_score)]
    pub fn get_show_score(&self) -> bool { self.show_score }
    #[gml::set(show_score)]
    pub fn set_show_score(&mut self, value: bool) { self.show_score = value; }

    #[gml::get(show_lives)]
    pub fn get_show_lives(&self) -> bool { self.show_lives }
    #[gml::set(show_lives)]
    pub fn set_show_lives(&mut self, value: bool) { self.show_lives = value; }

    #[gml::get(show_health)]
    pub fn get_show_health(&self) -> bool { self.show_health }
    #[gml::set(show_health)]
    pub fn set_show_health(&mut self, value: bool) { self.show_health = value; }

    #[gml::get(caption_score)]
    pub fn get_caption_score(&self) -> Symbol { self.caption_score }
    #[gml::set(caption_score)]
    pub fn set_caption_score(&mut self, value: Symbol) { self.caption_score = value; }

    #[gml::get(caption_lives)]
    pub fn get_caption_lives(&self) -> Symbol { self.caption_lives }
    #[gml::set(caption_lives)]
    pub fn set_caption_lives(&mut self, value: Symbol) { self.caption_lives = value; }

    #[gml::get(caption_health)]
    pub fn get_caption_health(&self) -> Symbol { self.caption_health }
    #[gml::set(caption_health)]
    pub fn set_caption_health(&mut self, value: Symbol) { self.caption_health = value; }

    #[gml::api]
    pub fn action_set_score(&mut self, relative: bool, mut value: f32) {
        if relative {
            value += self.get_score();
        }
        self.set_score(value)
    }
    #[gml::api]
    pub fn action_draw_score(
        &mut self, relative: bool,
        x: f32, y: f32, caption: Symbol
    ) {
        let _ = (relative, x, y, caption);
    }

    #[gml::api]
    pub fn action_set_life(&mut self, relative: bool, mut value: f32) {
        if relative {
            value += self.get_lives();
        }
        self.set_lives(value)
    }
    #[gml::api]
    pub fn action_draw_life_images(
        &mut self, relative: bool,
        x: f32, y: f32, image: i32
    ) {
        let _ = (relative, x, y, image);
    }

    #[gml::api]
    pub fn action_set_health(&mut self, relative: bool, mut value: f32) {
        if relative {
            value += self.get_health();
        }
        self.set_health(value)
    }
    #[gml::api]
    pub fn action_draw_health(
        &mut self, relative: bool,
        x1: f32, y1: f32, x2: f32, y2: f32, back: u32, bar: u32,
    ) {
        let _ = (relative, x1, y1, x2, y2, back, bar);
    }

    #[gml::api]
    pub fn action_set_caption(
        &mut self,
        show_score: bool, caption_score: Symbol,
        show_lives: bool, caption_lives: Symbol,
        show_health: bool, caption_health: Symbol,
    ) {
        self.set_show_score(show_score);
        self.set_caption_score(caption_score);
        self.set_show_lives(show_lives);
        self.set_caption_lives(caption_lives);
        self.set_show_health(show_health);
        self.set_caption_health(caption_health);
    }
}
