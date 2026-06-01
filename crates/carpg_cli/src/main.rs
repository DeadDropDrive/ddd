use std::error::Error;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ddd_core::{
    starter_car_catalog, starter_jobs, Car, ForkChoicePreview, ForkResolution, Garage,
    InstalledUpgrade, Job, LootEffect, LootItem, LootRarity, PatrolActionPreview, PatrolResolution,
    RouteAction, RouteEncounterPreview, RouteEvent, RoutePhase, RouteResolution, RouteRun,
    RouteRunReport, SegmentOutcome, SegmentResolution, UpgradeEffect,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), Box<dyn Error>> {
    let mut app = App::new();

    while !app.quit {
        terminal.draw(|frame| app.draw(frame))?;
        app.tick();

        if event::poll(Duration::from_millis(120))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Press {
                app.handle_key(key.code);
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Start,
    Garage,
    BlackMarket,
    CarMarket,
    Mechanic,
    Contracts,
    Route,
    PostRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartFocus {
    Name,
    Car,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GarageFocus {
    Cars,
    Inventory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MechanicFocus {
    AvailableParts,
    InstalledMods,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlackMarketFocus {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CarMarketFocus {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PostAction {
    RepairCar,
    Continue,
}

struct App {
    garage: Garage,
    jobs: Vec<Job>,
    player_name: String,
    starter_cars: Vec<Car>,
    mode: Mode,
    start_focus: StartFocus,
    garage_focus: GarageFocus,
    mechanic_focus: MechanicFocus,
    black_market_focus: BlackMarketFocus,
    car_market_focus: CarMarketFocus,
    selected_car: usize,
    selected_job: usize,
    selected_inventory: usize,
    selected_market_part: usize,
    selected_market_car: usize,
    selected_mechanic_part: usize,
    selected_upgrade: usize,
    selected_action: usize,
    route_run: Option<RouteRun>,
    route_roll_frame: u8,
    pending_roll: Option<PendingRoll>,
    last_segment_resolution: Option<SegmentResolution>,
    segment_outcomes: Vec<Option<bool>>,
    active_encounter_segment: Option<u8>,
    run_seed: u64,
    last_resolution: Option<RouteResolution>,
    last_report: Option<RouteRunReport>,
    last_car_index: Option<usize>,
    status: String,
    quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            garage: Garage::new(750, Vec::new()),
            jobs: starter_jobs(),
            player_name: String::new(),
            starter_cars: starter_car_catalog(),
            mode: Mode::Start,
            start_focus: StartFocus::Name,
            garage_focus: GarageFocus::Cars,
            mechanic_focus: MechanicFocus::AvailableParts,
            black_market_focus: BlackMarketFocus::Buy,
            car_market_focus: CarMarketFocus::Buy,
            selected_car: 0,
            selected_job: 0,
            selected_inventory: 0,
            selected_market_part: 0,
            selected_market_car: 0,
            selected_mechanic_part: 0,
            selected_upgrade: 0,
            selected_action: 0,
            route_run: None,
            route_roll_frame: 1,
            pending_roll: None,
            last_segment_resolution: None,
            segment_outcomes: Vec::new(),
            active_encounter_segment: None,
            run_seed: 11,
            last_resolution: None,
            last_report: None,
            last_car_index: None,
            status: "Enter a driver name and choose a starter car.".to_string(),
            quit: false,
        }
    }

    fn draw(&self, frame: &mut Frame) {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(12),
                Constraint::Length(3),
            ])
            .split(frame.area());

        frame.render_widget(self.header(), root[0]);
        match self.mode {
            Mode::Start => self.draw_start(frame, root[1]),
            Mode::Garage => self.draw_garage(frame, root[1]),
            Mode::BlackMarket => self.draw_black_market(frame, root[1]),
            Mode::CarMarket => self.draw_car_market(frame, root[1]),
            Mode::Mechanic => self.draw_mechanic(frame, root[1]),
            Mode::Contracts => self.draw_contracts(frame, root[1]),
            Mode::Route => self.draw_route(frame, root[1]),
            Mode::PostRun => self.draw_post_run(frame, root[1]),
        }
        frame.render_widget(self.footer(), root[2]);
    }

    fn header(&self) -> Paragraph<'_> {
        if self.mode == Mode::Start {
            let name = if self.player_name.trim().is_empty() {
                "new driver"
            } else {
                self.player_name.trim()
            };
            return Paragraph::new(format!("DDD | Start | {name} | {}", self.status))
                .block(Block::default().borders(Borders::ALL).title("Setup"));
        }

        Paragraph::new(format!(
            "DDD | {} | Cash ${} | Heat {} | Inventory {} | {}",
            self.player_name.trim(),
            self.garage.cash,
            self.garage.heat,
            self.garage.inventory.len(),
            self.status
        ))
        .block(Block::default().borders(Borders::ALL).title("Garage"))
    }

    fn footer(&self) -> Paragraph<'_> {
        let text = match self.mode {
            Mode::Start => "Name: type/backspace | Tab/Left/Right focus | Up/Down car | Enter next/confirm | q quit",
            Mode::Garage => {
                "Up/Down select car | Enter contracts | m mechanic | b parts | c cars | r repair | q quit"
            }
            Mode::BlackMarket => {
                "Left/Right buy/sell | Up/Down select | Enter trade | Esc garage | q quit"
            }
            Mode::CarMarket => {
                "Left/Right buy/sell | Up/Down select | Enter trade | Esc garage | q quit"
            }
            Mode::Mechanic => "Left/Right parts/mods | Up/Down select | Enter apply | u uninstall | Esc garage | q quit",
            Mode::Contracts => "Up/Down select contract | Enter dispatch | Esc garage | q quit",
            Mode::Route => match self.route_run.as_ref().map(|run| run.phase) {
                Some(RoutePhase::Segment) => "Right/Enter advance segment | q quit",
                _ => "Up/Down select action | Enter resolve | q quit",
            },
            Mode::PostRun => "Up/Down select post-run action | Enter apply | q quit",
        };
        Paragraph::new(text).block(Block::default().borders(Borders::ALL))
    }

    fn tick(&mut self) {
        let Some(pending) = &mut self.pending_roll else {
            return;
        };
        pending.ticks = pending.ticks.saturating_add(1);
        pending.frame_value = ((pending.frame_value as u16 + 29) % 100).max(1) as u8;
        self.route_roll_frame = pending.frame_value;
        if pending.ticks >= 16 {
            self.finish_pending_roll();
        }
    }

    fn finish_pending_roll(&mut self) {
        let Some(mut pending) = self.pending_roll.take() else {
            return;
        };
        pending.frame_value = pending.final_value;
        self.route_roll_frame = pending.final_value;
        match pending.result {
            PendingRollResult::Segment(resolution) => self.apply_segment_resolution(resolution),
            PendingRollResult::Encounter(resolution) => self.apply_route_resolution(resolution),
        }
    }

    fn draw_start(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new(self.start_name_lines())
                .block(Block::default().borders(Borders::ALL).title("Driver"))
                .wrap(Wrap { trim: true }),
            columns[0],
        );
        frame.render_widget(
            List::new(self.starter_car_items())
                .block(Block::default().borders(Borders::ALL).title("Starter Cars")),
            columns[1],
        );
        frame.render_widget(
            Paragraph::new(self.start_car_detail_lines())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Starter Setup"),
                )
                .wrap(Wrap { trim: true }),
            columns[2],
        );
    }

    fn draw_garage(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(13), Constraint::Length(6)])
            .split(area);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(rows[0]);

        frame.render_widget(
            List::new(self.car_items()).block(Block::default().borders(Borders::ALL).title("Cars")),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(self.car_detail_lines())
                .block(Block::default().borders(Borders::ALL).title("Performance"))
                .wrap(Wrap { trim: true }),
            columns[1],
        );
        frame.render_widget(
            Paragraph::new(self.garage_action_lines())
                .block(Block::default().borders(Borders::ALL).title("Garage"))
                .wrap(Wrap { trim: true }),
            columns[2],
        );
        frame.render_widget(
            List::new(self.inventory_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Inventory | trade at black market"),
            ),
            rows[1],
        );
    }

    fn draw_black_market(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(10)])
            .split(area);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[0]);

        frame.render_widget(
            List::new(self.black_market_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Buy Parts | Enter"),
            ),
            columns[0],
        );
        frame.render_widget(
            List::new(self.black_market_inventory_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Sell Inventory | Enter"),
            ),
            columns[1],
        );
        let info_columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[1]);
        frame.render_widget(
            Paragraph::new(self.black_market_part_lines())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Part Information"),
                )
                .wrap(Wrap { trim: true }),
            info_columns[0],
        );
        frame.render_widget(
            Paragraph::new(self.black_market_effect_lines())
                .block(Block::default().borders(Borders::ALL).title("Effect"))
                .wrap(Wrap { trim: true }),
            info_columns[1],
        );
    }

    fn draw_car_market(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(10)])
            .split(area);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[0]);

        frame.render_widget(
            List::new(self.car_market_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Buy Cars | Enter"),
            ),
            columns[0],
        );
        frame.render_widget(
            List::new(self.car_market_garage_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Sell Cars | Enter"),
            ),
            columns[1],
        );
        frame.render_widget(
            Paragraph::new(self.car_market_detail_lines())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Car Information"),
                )
                .wrap(Wrap { trim: true }),
            rows[1],
        );
    }

    fn draw_contracts(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(area);

        frame.render_widget(
            List::new(self.job_items())
                .block(Block::default().borders(Borders::ALL).title("Contracts")),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(self.contract_detail_lines())
                .block(Block::default().borders(Borders::ALL).title("Briefing"))
                .wrap(Wrap { trim: true }),
            columns[1],
        );
    }

    fn draw_mechanic(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new(self.mechanic_car_lines())
                .block(Block::default().borders(Borders::ALL).title("The Mechanic"))
                .wrap(Wrap { trim: true }),
            columns[0],
        );
        frame.render_widget(
            List::new(self.mechanic_part_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Parts | Enter install"),
            ),
            columns[1],
        );
        frame.render_widget(
            List::new(self.mechanic_upgrade_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Installed Mods | Enter/u uninstall"),
            ),
            columns[2],
        );
    }

    fn car_items(&self) -> Vec<ListItem<'_>> {
        self.garage
            .cars
            .iter()
            .enumerate()
            .map(|(index, car)| {
                let selected = index == self.selected_car;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", car.name)),
                        rarity_badge(car.rarity, true),
                    ]),
                    selected && self.mode == Mode::Garage && self.garage_focus == GarageFocus::Cars,
                )
            })
            .collect()
    }

    fn job_items(&self) -> Vec<ListItem<'_>> {
        self.jobs
            .iter()
            .enumerate()
            .map(|(index, job)| {
                let selected = index == self.selected_job;
                let marker = if selected { ">" } else { " " };
                let line = format!(
                    "{marker} {} | ${} | heat {}",
                    job.name, job.payout, job.heat
                );
                styled_item(line, selected && self.mode == Mode::Contracts)
            })
            .collect()
    }

    fn inventory_items(&self) -> Vec<ListItem<'_>> {
        if self.garage.inventory.is_empty() {
            return vec![ListItem::new("empty")];
        }

        self.garage
            .inventory
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let selected = index == self.selected_inventory;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", item.name)),
                        rarity_badge(item.rarity, true),
                        Span::raw(format!(
                            " | ${} | {}",
                            item.black_market_value,
                            loot_category_label(item)
                        )),
                    ]),
                    selected
                        && self.mode == Mode::Garage
                        && self.garage_focus == GarageFocus::Inventory,
                )
            })
            .collect()
    }

    fn black_market_items(&self) -> Vec<ListItem<'_>> {
        if self.garage.black_market.is_empty() {
            return vec![ListItem::new("no parts available")];
        }

        self.garage
            .black_market
            .iter()
            .enumerate()
            .map(|(index, offer)| {
                let selected = index == self.selected_market_part;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", offer.item.name)),
                        rarity_badge(offer.item.rarity, true),
                        Span::raw(format!(
                            " | buy ${} | sell ${}",
                            offer.asking_price, offer.item.black_market_value
                        )),
                    ]),
                    selected
                        && self.mode == Mode::BlackMarket
                        && self.black_market_focus == BlackMarketFocus::Buy,
                )
            })
            .collect()
    }

    fn black_market_inventory_items(&self) -> Vec<ListItem<'_>> {
        if self.garage.inventory.is_empty() {
            return vec![ListItem::new("nothing to sell")];
        }

        self.garage
            .inventory
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let selected = index == self.selected_inventory;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", item.name)),
                        rarity_badge(item.rarity, true),
                        Span::raw(format!(" | sell ${}", item.black_market_value)),
                    ]),
                    selected
                        && self.mode == Mode::BlackMarket
                        && self.black_market_focus == BlackMarketFocus::Sell,
                )
            })
            .collect()
    }

    fn mechanic_part_items(&self) -> Vec<ListItem<'_>> {
        let upgrades = self.vehicle_upgrade_inventory_items();
        if upgrades.is_empty() {
            return vec![ListItem::new("no vehicle upgrades in inventory")];
        }

        upgrades
            .into_iter()
            .enumerate()
            .map(|(part_index, (_inventory_index, item))| {
                let selected = part_index == self.selected_mechanic_part;
                let marker = if selected { ">" } else { " " };
                mechanic_part_item(
                    marker,
                    item,
                    selected
                        && self.mode == Mode::Mechanic
                        && self.mechanic_focus == MechanicFocus::AvailableParts,
                )
            })
            .collect()
    }

    fn vehicle_upgrade_inventory_items(&self) -> Vec<(usize, &LootItem)> {
        self.garage
            .inventory
            .iter()
            .enumerate()
            .filter(|(_, item)| is_vehicle_upgrade(item))
            .collect()
    }

    fn mechanic_upgrade_items(&self) -> Vec<ListItem<'_>> {
        let car = &self.garage.cars[self.selected_car];
        if car.installed_upgrades.is_empty() {
            return vec![ListItem::new("none installed on selected car")];
        }

        car.installed_upgrades
            .iter()
            .enumerate()
            .map(|(index, upgrade)| {
                let selected = index == self.selected_upgrade;
                let marker = if selected { ">" } else { " " };
                mechanic_upgrade_item(
                    marker,
                    upgrade,
                    selected
                        && self.mode == Mode::Mechanic
                        && self.mechanic_focus == MechanicFocus::InstalledMods,
                )
            })
            .collect()
    }

    fn start_name_lines(&self) -> Vec<Line<'_>> {
        let focused = self.start_focus == StartFocus::Name;
        let name = if self.player_name.is_empty() {
            "type name".to_string()
        } else if focused {
            format!("{}_", self.player_name)
        } else {
            self.player_name.clone()
        };

        vec![
            title_line("Name"),
            styled_line(name, focused),
            Line::from(""),
            subdued_line("This will identify the player in the garage header.".to_string()),
            subdued_line("More driver flavor can attach here later.".to_string()),
        ]
    }

    fn starter_car_items(&self) -> Vec<ListItem<'_>> {
        if self.starter_cars.is_empty() {
            return vec![ListItem::new("no starter cars configured")];
        }

        self.starter_cars
            .iter()
            .enumerate()
            .map(|(index, car)| {
                let selected = index == self.selected_car;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", car.name)),
                        rarity_badge(car.rarity, true),
                    ]),
                    selected && self.mode == Mode::Start && self.start_focus == StartFocus::Car,
                )
            })
            .collect()
    }

    fn car_market_items(&self) -> Vec<ListItem<'_>> {
        if self.garage.car_market.is_empty() {
            return vec![ListItem::new("no cars available")];
        }

        self.garage
            .car_market
            .iter()
            .enumerate()
            .map(|(index, offer)| {
                let selected = index == self.selected_market_car;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", offer.car.name)),
                        rarity_badge(offer.car.rarity, true),
                        Span::raw(format!(
                            " | buy ${} | sell ${}",
                            offer.asking_price,
                            self.garage.estimated_car_sale_value(&offer.car)
                        )),
                    ]),
                    selected
                        && self.mode == Mode::CarMarket
                        && self.car_market_focus == CarMarketFocus::Buy,
                )
            })
            .collect()
    }

    fn car_market_garage_items(&self) -> Vec<ListItem<'_>> {
        if self.garage.cars.is_empty() {
            return vec![ListItem::new("no cars owned")];
        }

        self.garage
            .cars
            .iter()
            .enumerate()
            .map(|(index, car)| {
                let selected = index == self.selected_car;
                let marker = if selected { ">" } else { " " };
                styled_line_item(
                    Line::from(vec![
                        Span::raw(format!("{marker} {} ", car.name)),
                        rarity_badge(car.rarity, true),
                        Span::raw(format!(
                            " | sell ${}",
                            self.garage.car_sale_value(index).unwrap_or(0)
                        )),
                    ]),
                    selected
                        && self.mode == Mode::CarMarket
                        && self.car_market_focus == CarMarketFocus::Sell,
                )
            })
            .collect()
    }

    fn start_car_detail_lines(&self) -> Vec<Line<'_>> {
        let Some(car) = self.starter_cars.get(self.selected_car) else {
            return vec![
                title_line("NO STARTERS"),
                subdued_line("Mark at least one car as starter_eligible in cars.tsv.".to_string()),
            ];
        };

        let mut lines = car_performance_lines(car);
        lines.push(Line::from(""));
        lines.push(title_line("Start"));
        lines.push(styled_line(
            ">> START GAME <<".to_string(),
            self.start_focus == StartFocus::Confirm,
        ));
        lines.push(subdued_line(
            "Enter confirms this driver and starter car.".to_string(),
        ));
        lines
    }

    fn car_detail_lines(&self) -> Vec<Line<'_>> {
        let car = &self.garage.cars[self.selected_car];
        car_performance_lines(car)
    }

    fn garage_action_lines(&self) -> Vec<Line<'_>> {
        vec![
            title_line("Selected car"),
            Line::from(self.garage.cars[self.selected_car].name.clone()),
            Line::from(""),
            title_line("Actions"),
            Line::from("Enter: choose contract"),
            Line::from("m: open mechanic"),
            Line::from("b: black market trade"),
            Line::from("r: repair selected car"),
        ]
    }

    fn black_market_part_lines(&self) -> Vec<Line<'_>> {
        if self.black_market_focus == BlackMarketFocus::Sell {
            return self.black_market_sell_part_lines();
        }

        let Some(offer) = self.garage.black_market.get(self.selected_market_part) else {
            return vec![
                title_line("NO STOCK"),
                subdued_line("The contact has no parts available right now.".to_string()),
                subdued_line("Stock refreshes after each run.".to_string()),
            ];
        };

        vec![
            Line::from(vec![
                Span::raw(format!("{} ", offer.item.name)),
                rarity_badge(offer.item.rarity, false),
            ]),
            Line::from(""),
            market_price_line("Buy price", offer.asking_price),
            subdued_line(format!("Sell value: ${}", offer.item.black_market_value)),
            subdued_line(format!(
                "Markup: +${}",
                offer.asking_price - offer.item.black_market_value
            )),
            Line::from(""),
            subdued_line("Enter buys into inventory.".to_string()),
            subdued_line("Stock refreshes after each run.".to_string()),
        ]
    }

    fn black_market_sell_part_lines(&self) -> Vec<Line<'_>> {
        let Some(item) = self.garage.inventory.get(self.selected_inventory) else {
            return vec![
                title_line("NO INVENTORY"),
                subdued_line("Nothing in inventory to sell.".to_string()),
                subdued_line("Complete runs or buy parts to build stock.".to_string()),
            ];
        };

        vec![
            Line::from(vec![
                Span::raw(format!("{} ", item.name)),
                rarity_badge(item.rarity, false),
            ]),
            Line::from(""),
            market_price_line("Sell value", item.black_market_value),
            Line::from(""),
            subdued_line("Enter sells this item.".to_string()),
        ]
    }

    fn black_market_effect_lines(&self) -> Vec<Line<'_>> {
        let effect = match self.black_market_focus {
            BlackMarketFocus::Buy => self
                .garage
                .black_market
                .get(self.selected_market_part)
                .map(|offer| &offer.item.effect),
            BlackMarketFocus::Sell => self
                .garage
                .inventory
                .get(self.selected_inventory)
                .map(|item| &item.effect),
        };

        let Some(effect) = effect else {
            return vec![subdued_line("No part selected.".to_string())];
        };

        let mut lines = Vec::new();
        append_loot_effect_lines(&mut lines, effect);
        lines
    }

    fn car_market_detail_lines(&self) -> Vec<Line<'_>> {
        match self.car_market_focus {
            CarMarketFocus::Buy => {
                let Some(offer) = self.garage.car_market.get(self.selected_market_car) else {
                    return vec![
                        title_line("NO STOCK"),
                        subdued_line("No cars are available right now.".to_string()),
                        subdued_line("Stock refreshes after each run.".to_string()),
                    ];
                };
                let mut lines = car_trade_lines(&offer.car);
                lines.push(Line::from(""));
                market_price_line_into(&mut lines, "Buy price", offer.asking_price);
                lines.push(subdued_line(format!(
                    "Estimated sell value: ${}",
                    self.garage.estimated_car_sale_value(&offer.car)
                )));
                lines.push(subdued_line(format!(
                    "Markup: +${}",
                    offer.asking_price - offer.car.value
                )));
                lines
            }
            CarMarketFocus::Sell => {
                let Some(car) = self.garage.cars.get(self.selected_car) else {
                    return vec![subdued_line("No car selected.".to_string())];
                };
                let mut lines = car_trade_lines(car);
                lines.push(Line::from(""));
                market_price_line_into(
                    &mut lines,
                    "Sell value",
                    self.garage.car_sale_value(self.selected_car).unwrap_or(0),
                );
                lines.push(subdued_line(format!(
                    "Condition: {}% | Heat: {}",
                    car.condition.average(),
                    self.garage.heat
                )));
                if !car.installed_upgrades.is_empty() {
                    lines.push(subdued_line(
                        "Installed upgrades add 50% of value plus install cost.".to_string(),
                    ));
                }
                if self.garage.cars.len() <= 1 {
                    lines.push(subdued_line("You cannot sell your last car.".to_string()));
                }
                lines
            }
        }
    }

    fn contract_detail_lines(&self) -> Vec<Line<'_>> {
        let car = &self.garage.cars[self.selected_car];
        let job = &self.jobs[self.selected_job];
        vec![
            title_line("Driver"),
            Line::from(car.name.clone()),
            Line::from(""),
            title_line("Contract"),
            Line::from(job.name.clone()),
            compact_stat_line(
                "Pay",
                format!("${}", job.payout),
                "Heat",
                job.heat.to_string(),
            ),
            compact_stat_line(
                "Cargo",
                format!("{:?} {}", job.cargo, job.cargo_size),
                "Terrain",
                format!("{:?}", job.terrain),
            ),
            compact_stat_line(
                "Distance",
                job.distance.to_string(),
                "Capacity",
                car.cargo_capacity.to_string(),
            ),
            Line::from(""),
            Line::from("Press Enter to dispatch this car on this contract."),
        ]
    }

    fn mechanic_car_lines(&self) -> Vec<Line<'_>> {
        let mut lines = self.car_detail_lines();
        let car = &self.garage.cars[self.selected_car];
        lines.push(Line::from(""));
        lines.push(title_line("Condition"));
        lines.push(Line::from(format!(
            "engine {} | suspension {}",
            car.condition.engine, car.condition.suspension
        )));
        lines.push(Line::from(format!(
            "tires {} | body {}",
            car.condition.tires, car.condition.body
        )));
        if self.install_cost_voucher_available() {
            lines.push(Line::from(""));
            lines.push(install_cost_line(0));
            lines.push(subdued_line(
                "Voucher available: next install cost is waived".to_string(),
            ));
        }
        lines
    }

    fn install_cost_voucher_available(&self) -> bool {
        self.garage
            .inventory
            .iter()
            .any(|item| matches!(item.effect, LootEffect::InstallCostVoucher(_)))
    }

    fn draw_route(&self, frame: &mut Frame, area: Rect) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(5), Constraint::Min(8)])
            .split(area);

        frame.render_widget(
            Paragraph::new(self.route_strip_lines(rows[0].width))
                .block(Block::default().borders(Borders::ALL).title("Route"))
                .wrap(Wrap { trim: true }),
            rows[0],
        );

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(rows[1]);
        let side = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(6)])
            .split(columns[1]);

        let preview_lines = self.route_preview_lines();
        frame.render_widget(
            Paragraph::new(preview_lines)
                .block(Block::default().borders(Borders::ALL).title("Encounter"))
                .wrap(Wrap { trim: true }),
            columns[0],
        );

        frame.render_widget(
            Paragraph::new(self.dice_roll_lines())
                .block(Block::default().borders(Borders::ALL).title("Dice"))
                .wrap(Wrap { trim: true }),
            side[0],
        );

        frame.render_widget(
            Paragraph::new(self.last_resolution_lines())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Last Resolution"),
                )
                .wrap(Wrap { trim: true }),
            side[1],
        );
    }

    fn route_strip_lines(&self, width: u16) -> Vec<Line<'_>> {
        let Some(route_run) = &self.route_run else {
            return vec![Line::from("No active route.")];
        };
        let phase = if self.pending_roll.is_some() {
            "travel"
        } else {
            match route_run.phase {
                RoutePhase::Segment => "travel",
                RoutePhase::Patrol => "patrol encounter",
                RoutePhase::Fork => "route choice",
                RoutePhase::Backtrack => "backtracking",
                RoutePhase::Complete => "complete",
            }
        };
        let progress_percent = self
            .pending_roll
            .as_ref()
            .filter(|pending| pending.is_segment_roll())
            .map(PendingRoll::progress_percent)
            .unwrap_or(0);
        let display_segment_index = self
            .pending_segment_resolution()
            .map(|resolution| resolution.segment_index)
            .unwrap_or(route_run.current_segment)
            .min(route_run.job.distance);
        let display_segment_number = if display_segment_index >= route_run.job.distance {
            route_run.job.distance
        } else {
            display_segment_index.saturating_add(1)
        };
        let track = route_track_line(
            display_segment_index,
            route_run.job.distance,
            width.saturating_sub(4),
            progress_percent,
            &self.segment_outcomes,
            self.active_encounter_segment,
        );
        vec![
            track,
            Line::from(format!(
                "Segment {}/{} | {}",
                display_segment_number, route_run.job.distance, phase
            )),
        ]
    }

    fn pending_segment_resolution(&self) -> Option<&SegmentResolution> {
        let pending = self.pending_roll.as_ref()?;
        match &pending.result {
            PendingRollResult::Segment(resolution) => Some(resolution),
            PendingRollResult::Encounter(_) => None,
        }
    }

    fn route_preview_lines(&self) -> Vec<Line<'_>> {
        if self
            .pending_roll
            .as_ref()
            .is_some_and(PendingRoll::is_encounter_roll)
        {
            return vec![subdued_line("rolling...".to_string())];
        }
        if self.pending_roll.is_some() {
            return vec![subdued_line("driving...".to_string())];
        }

        let Some(route_run) = &self.route_run else {
            return vec![Line::from("No active route.")];
        };
        let Some(car) = self.garage.cars.get(route_run.car_index) else {
            return vec![Line::from("Car lost.")];
        };
        let effective = car.effective_stats();
        let mut lines = vec![
            Line::from(format!(
                "{} on {} | {:?} {:?}",
                car.name, route_run.job.name, route_run.job.cargo, route_run.job.terrain
            )),
            Line::from(format!(
                "Garage heat {} | stealth {} | drivetrain {:+} | condition {}%",
                self.garage.heat,
                effective.stealth,
                effective.drivetrain,
                car.condition.average()
            )),
            Line::from(""),
        ];

        if route_run.phase == RoutePhase::Segment {
            lines.extend(self.route_segment_lines(route_run));
            return lines;
        }

        match route_run.encounter_preview(&self.garage) {
            Some(RouteEncounterPreview::Patrol(previews)) => {
                lines.push(title_line("PATROL ENCOUNTER"));
                lines.extend(previews.iter().enumerate().flat_map(|(index, preview)| {
                    preview_lines_for_patrol(index, preview, index == self.selected_action)
                }));
            }
            Some(RouteEncounterPreview::Fork(previews)) => {
                lines.push(title_line("FORK IN THE ROAD"));
                lines.extend(previews.iter().enumerate().flat_map(|(index, preview)| {
                    preview_lines_for_fork(index, preview, index == self.selected_action)
                }));
            }
            None => lines.push(Line::from("Route is ready to complete.")),
        }

        lines
    }

    fn route_segment_lines(&self, route_run: &RouteRun) -> Vec<Line<'_>> {
        let mut lines = Vec::new();
        if let Some(preview) = route_run.segment_preview(&self.garage) {
            lines.push(Line::from(format!(
                "Segment {}/{} | encounter {}% | crash {}% | cops {}%",
                preview.segment_index + 1,
                preview.segment_count,
                preview.encounter_chance,
                preview.crash_chance,
                preview.caught_chance
            )));
            lines.push(Line::from(""));
            lines.push(subdued_line(
                "Press Right or Enter to drive this segment.".to_string(),
            ));
        } else {
            lines.push(title_line("ROUTE COMPLETE"));
            lines.push(Line::from("Route is ready to settle."));
        }

        if let Some(resolution) = &self.last_segment_resolution {
            lines.push(Line::from(""));
            lines.push(title_line("LAST SEGMENT"));
            lines.push(Line::from(format!(
                "Segment {}/{} | target encounter {}% | crash {}%",
                resolution.segment_index + 1,
                resolution.segment_count,
                resolution.encounter_chance,
                resolution.crash_chance
            )));
            lines.push(Line::from(format!(
                "Outcome: {}",
                describe_segment_outcome(resolution.outcome)
            )));
            for event in &resolution.events {
                lines.push(subdued_line(describe_event(event)));
            }
        }
        lines
    }

    fn last_resolution_lines(&self) -> Vec<Line<'static>> {
        if let Some(resolution) = &self.last_resolution {
            return resolution_lines(resolution);
        }
        if let Some(resolution) = &self.last_segment_resolution {
            return segment_resolution_lines(resolution);
        }
        vec![subdued_line("no resolved roll yet".to_string())]
    }

    fn dice_roll_lines(&self) -> Vec<Line<'static>> {
        if let Some(pending) = &self.pending_roll {
            if pending.is_segment_roll() {
                return vec![
                    title_line("TRAVEL"),
                    Line::from(Span::styled(
                        format!("      {:>3}%", pending.progress_percent()),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )),
                ];
            }
            let value = if pending.ticks >= 15 {
                pending.final_value
            } else {
                pending.frame_value
            };
            let color = if pending.ticks >= 15 {
                if pending.success {
                    Color::Green
                } else {
                    Color::Red
                }
            } else {
                Color::Cyan
            };
            return vec![
                title_line("D100"),
                Line::from(Span::styled(
                    format!("      {:03}", value),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )),
                subdued_line(pending.label.clone()),
            ];
        }

        vec![
            title_line("D100"),
            Line::from(Span::styled(
                format!("      {:03}", self.route_roll_frame),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )),
            subdued_line("waiting for next roll".to_string()),
        ]
    }

    fn draw_post_run(&self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
            .split(area);

        frame.render_widget(
            List::new(self.post_action_items()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Post-Run Actions"),
            ),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(self.post_run_summary_lines())
                .block(Block::default().borders(Borders::ALL).title("Run Summary"))
                .wrap(Wrap { trim: true }),
            columns[1],
        );
    }

    fn post_action_items(&self) -> Vec<ListItem<'_>> {
        self.post_actions()
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let selected = index == self.selected_action;
                let marker = if selected { ">" } else { " " };
                styled_item(format!("{marker} {}", post_action_label(*action)), selected)
            })
            .collect()
    }

    fn post_run_summary_lines(&self) -> Vec<Line<'_>> {
        let mut lines = Vec::new();
        if let Some(report) = &self.last_report {
            if report.car_wrecked {
                lines.push(outcome_line(
                    "Critical failure",
                    "car wrecked and removed from garage",
                    Color::Red,
                ));
                lines.push(Line::from(""));
            }
            if report.caught_by_cops {
                lines.push(outcome_line(
                    "Critical failure",
                    "caught by cops; car impounded",
                    Color::Red,
                ));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(format!(
                "Completed: {} | payout ${} | heat gained {}",
                report.outcome.completed, report.outcome.payout, report.outcome.heat_gained
            )));
            if report.payout_multiplier_percent < 100 {
                lines.push(Line::from(format!(
                    "Payout multiplier: {}%",
                    report.payout_multiplier_percent
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from("Route log"));
            lines.extend(
                report
                    .outcome
                    .events
                    .iter()
                    .map(|event| Line::from(format!("- {}", describe_event(event)))),
            );
            lines.push(Line::from(""));
            lines.push(Line::from("Loot"));
            if report.loot.is_empty() {
                lines.push(Line::from("- none"));
            } else {
                lines.extend(report.loot.iter().map(loot_summary_line));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "Inventory value: ${}",
            self.inventory_value()
        )));
        if let Some(car_index) = self.last_car_index {
            let car = &self.garage.cars[car_index];
            lines.push(Line::from(format!(
                "{} repair cost: ${}",
                car.name,
                car.repair_cost()
            )));
        }
        lines
    }

    fn handle_key(&mut self, code: KeyCode) {
        if self.pending_roll.is_some() {
            if code == KeyCode::Char('q') {
                self.quit = true;
            }
            return;
        }
        if self.mode == Mode::Start {
            self.handle_start_key(code);
            return;
        }
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('r') => self.repair_selected_car_from_garage(),
            KeyCode::Char('m') => self.open_mechanic(),
            KeyCode::Char('b') => self.open_black_market(),
            KeyCode::Char('c') => self.open_car_market(),
            KeyCode::Char('u') => self.uninstall_selected_upgrade_from_garage(),
            KeyCode::Esc => self.handle_escape(),
            KeyCode::Left | KeyCode::BackTab => self.previous_focus(),
            KeyCode::Right | KeyCode::Tab => self.handle_right(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Enter => self.activate_selection(),
            _ => {}
        }
    }

    fn handle_escape(&mut self) {
        match self.mode {
            Mode::BlackMarket | Mode::CarMarket | Mode::Mechanic | Mode::Contracts => {
                self.mode = Mode::Garage;
                self.selected_action = 0;
                self.status = "Back at the garage.".to_string();
            }
            _ => {}
        }
    }

    fn handle_start_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char(ch) if self.start_focus == StartFocus::Name => {
                if self.player_name.len() < 24
                    && (ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_'))
                {
                    self.player_name.push(ch);
                }
            }
            KeyCode::Backspace if self.start_focus == StartFocus::Name => {
                self.player_name.pop();
            }
            KeyCode::Left | KeyCode::BackTab => self.previous_focus(),
            KeyCode::Right | KeyCode::Tab => self.next_focus(),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Enter => self.activate_selection(),
            _ => {}
        }
    }

    fn previous_focus(&mut self) {
        match self.mode {
            Mode::Start => {
                self.start_focus = match self.start_focus {
                    StartFocus::Name => StartFocus::Confirm,
                    StartFocus::Car => StartFocus::Name,
                    StartFocus::Confirm => StartFocus::Car,
                };
            }
            Mode::Garage => {
                self.garage_focus = match self.garage_focus {
                    GarageFocus::Cars => GarageFocus::Inventory,
                    GarageFocus::Inventory => GarageFocus::Cars,
                };
            }
            Mode::Mechanic => {
                self.mechanic_focus = match self.mechanic_focus {
                    MechanicFocus::AvailableParts => MechanicFocus::InstalledMods,
                    MechanicFocus::InstalledMods => MechanicFocus::AvailableParts,
                };
            }
            Mode::BlackMarket => {
                self.black_market_focus = match self.black_market_focus {
                    BlackMarketFocus::Buy => BlackMarketFocus::Sell,
                    BlackMarketFocus::Sell => BlackMarketFocus::Buy,
                };
            }
            Mode::CarMarket => {
                self.car_market_focus = match self.car_market_focus {
                    CarMarketFocus::Buy => CarMarketFocus::Sell,
                    CarMarketFocus::Sell => CarMarketFocus::Buy,
                };
            }
            _ => {}
        }
    }

    fn next_focus(&mut self) {
        if self.mode == Mode::Start {
            self.start_focus = match self.start_focus {
                StartFocus::Name => StartFocus::Car,
                StartFocus::Car => StartFocus::Confirm,
                StartFocus::Confirm => StartFocus::Name,
            };
        } else {
            self.previous_focus();
        }
    }

    fn handle_right(&mut self) {
        if self.mode == Mode::Route
            && matches!(
                self.route_run.as_ref().map(|run| run.phase),
                Some(RoutePhase::Segment)
            )
        {
            self.resolve_route_segment();
        } else {
            self.next_focus();
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let max = match self.mode {
            Mode::Start => match self.start_focus {
                StartFocus::Car => self.starter_cars.len(),
                StartFocus::Name | StartFocus::Confirm => 0,
            },
            Mode::Garage => match self.garage_focus {
                GarageFocus::Cars => self.garage.cars.len(),
                GarageFocus::Inventory => self.garage.inventory.len(),
            },
            Mode::BlackMarket => match self.black_market_focus {
                BlackMarketFocus::Buy => self.garage.black_market.len(),
                BlackMarketFocus::Sell => self.garage.inventory.len(),
            },
            Mode::CarMarket => match self.car_market_focus {
                CarMarketFocus::Buy => self.garage.car_market.len(),
                CarMarketFocus::Sell => self.garage.cars.len(),
            },
            Mode::Mechanic => match self.mechanic_focus {
                MechanicFocus::AvailableParts => self.vehicle_upgrade_inventory_items().len(),
                MechanicFocus::InstalledMods => {
                    self.garage.cars[self.selected_car].installed_upgrades.len()
                }
            },
            Mode::Contracts => self.jobs.len(),
            Mode::Route => self.route_action_count(),
            Mode::PostRun => self.post_actions().len(),
        };
        if max == 0 {
            return;
        }

        let selected = match self.mode {
            Mode::Start => &mut self.selected_car,
            Mode::Garage => match self.garage_focus {
                GarageFocus::Cars => &mut self.selected_car,
                GarageFocus::Inventory => &mut self.selected_inventory,
            },
            Mode::BlackMarket => match self.black_market_focus {
                BlackMarketFocus::Buy => &mut self.selected_market_part,
                BlackMarketFocus::Sell => &mut self.selected_inventory,
            },
            Mode::CarMarket => match self.car_market_focus {
                CarMarketFocus::Buy => &mut self.selected_market_car,
                CarMarketFocus::Sell => &mut self.selected_car,
            },
            Mode::Mechanic => match self.mechanic_focus {
                MechanicFocus::AvailableParts => &mut self.selected_mechanic_part,
                MechanicFocus::InstalledMods => &mut self.selected_upgrade,
            },
            Mode::Contracts => &mut self.selected_job,
            _ => &mut self.selected_action,
        };
        *selected = wrap_index(*selected, max, delta);
        if self.mode == Mode::Garage && self.garage_focus == GarageFocus::Cars {
            self.selected_upgrade = self.selected_upgrade.min(
                self.garage.cars[self.selected_car]
                    .installed_upgrades
                    .len()
                    .saturating_sub(1),
            );
        }
    }

    fn activate_selection(&mut self) {
        match self.mode {
            Mode::Start => match self.start_focus {
                StartFocus::Name => self.start_focus = StartFocus::Car,
                StartFocus::Car => self.start_focus = StartFocus::Confirm,
                StartFocus::Confirm => self.start_game(),
            },
            Mode::Garage => match self.garage_focus {
                GarageFocus::Cars => self.open_contracts(),
                GarageFocus::Inventory => {
                    self.status = "Open the mechanic to install parts.".to_string()
                }
            },
            Mode::BlackMarket => match self.black_market_focus {
                BlackMarketFocus::Buy => self.buy_selected_black_market_part(),
                BlackMarketFocus::Sell => self.sell_selected_black_market_item(),
            },
            Mode::CarMarket => match self.car_market_focus {
                CarMarketFocus::Buy => self.buy_selected_market_car(),
                CarMarketFocus::Sell => self.sell_selected_market_car(),
            },
            Mode::Mechanic => match self.mechanic_focus {
                MechanicFocus::AvailableParts => self.install_selected_inventory_item(),
                MechanicFocus::InstalledMods => self.uninstall_selected_upgrade_from_garage(),
            },
            Mode::Contracts => self.dispatch_route(),
            Mode::Route => {
                if matches!(
                    self.route_run.as_ref().map(|run| run.phase),
                    Some(RoutePhase::Segment)
                ) {
                    self.resolve_route_segment();
                } else {
                    self.resolve_route_action();
                }
            }
            Mode::PostRun => self.apply_post_action(),
        }
    }

    fn start_game(&mut self) {
        let name = self.player_name.trim();
        if name.is_empty() {
            self.status = "Enter a driver name before starting.".to_string();
            self.start_focus = StartFocus::Name;
            return;
        }
        let Some(car) = self.starter_cars.get(self.selected_car).cloned() else {
            self.status = "No starter car is configured.".to_string();
            self.start_focus = StartFocus::Car;
            return;
        };

        self.player_name = name.to_string();
        self.garage = Garage::new(750, vec![car.clone()]);
        self.mode = Mode::Garage;
        self.garage_focus = GarageFocus::Cars;
        self.selected_car = 0;
        self.status = format!(
            "{} starts with {}. Choose a contract or visit the mechanic.",
            self.player_name, car.name
        );
    }

    fn open_contracts(&mut self) {
        self.mode = Mode::Contracts;
        self.status = format!(
            "Choose a contract for {}.",
            self.garage.cars[self.selected_car].name
        );
    }

    fn open_black_market(&mut self) {
        if self.mode != Mode::Garage {
            return;
        }
        self.mode = Mode::BlackMarket;
        self.black_market_focus = BlackMarketFocus::Buy;
        self.selected_market_part = self
            .selected_market_part
            .min(self.garage.black_market.len().saturating_sub(1));
        self.selected_inventory = self
            .selected_inventory
            .min(self.garage.inventory.len().saturating_sub(1));
        self.status = "Black market contact is open for buying and selling.".to_string();
    }

    fn open_car_market(&mut self) {
        if self.mode != Mode::Garage {
            return;
        }
        self.mode = Mode::CarMarket;
        self.car_market_focus = CarMarketFocus::Buy;
        self.selected_market_car = self
            .selected_market_car
            .min(self.garage.car_market.len().saturating_sub(1));
        self.selected_car = self
            .selected_car
            .min(self.garage.cars.len().saturating_sub(1));
        self.status = "Car market is open for buying and selling.".to_string();
    }

    fn open_mechanic(&mut self) {
        if self.mode != Mode::Garage {
            return;
        }
        self.mode = Mode::Mechanic;
        self.mechanic_focus = MechanicFocus::AvailableParts;
        self.status = format!(
            "Mechanic inspecting {}.",
            self.garage.cars[self.selected_car].name
        );
    }

    fn dispatch_route(&mut self) {
        let job = self.jobs[self.selected_job].clone();
        self.segment_outcomes = vec![None; job.distance as usize];
        self.route_run = Some(RouteRun::new(self.selected_car, job.clone(), self.run_seed));
        self.run_seed = self.run_seed.wrapping_add(53);
        self.last_resolution = None;
        self.last_segment_resolution = None;
        self.last_report = None;
        self.last_car_index = Some(self.selected_car);
        self.selected_action = 0;
        self.route_roll_frame = 1;
        self.pending_roll = None;
        self.active_encounter_segment = None;
        self.mode = Mode::Route;
        self.status = format!(
            "Dispatched {} on {}.",
            self.garage.cars[self.selected_car].name, job.name
        );
    }

    fn resolve_route_action(&mut self) {
        let Some(route_run) = &mut self.route_run else {
            return;
        };
        if route_run.phase == RoutePhase::Segment {
            return;
        }
        let Some(preview) = route_run.encounter_preview(&self.garage) else {
            self.complete_route();
            return;
        };

        let action = match preview {
            RouteEncounterPreview::Patrol(previews) => previews
                .get(self.selected_action)
                .map(|preview| RouteAction::Patrol(preview.action)),
            RouteEncounterPreview::Fork(previews) => previews
                .get(self.selected_action)
                .map(|preview| RouteAction::Fork(preview.choice)),
        };

        let Some(action) = action else {
            return;
        };
        let Some(resolution) = route_run.resolve_action(&mut self.garage, action) else {
            return;
        };
        let final_value = route_resolution_roll(&resolution).unwrap_or(1);
        let success = route_resolution_success(&resolution);
        self.pending_roll = Some(PendingRoll {
            label: route_resolution_label(&resolution).to_string(),
            final_value,
            success,
            ticks: 0,
            frame_value: self.route_roll_frame,
            result: PendingRollResult::Encounter(resolution),
        });
    }

    fn apply_route_resolution(&mut self, resolution: RouteResolution) {
        let Some(route_run) = &mut self.route_run else {
            return;
        };
        if let Some(segment_index) = self.active_encounter_segment.take() {
            if let Some(outcome) = self.segment_outcomes.get_mut(segment_index as usize) {
                *outcome = Some(route_resolution_success(&resolution));
            }
        }
        self.last_resolution = Some(resolution);
        self.selected_action = 0;

        if route_run.encounter_preview(&self.garage).is_none() {
            self.route_roll_frame = 1;
            if matches!(route_run.phase, RoutePhase::Segment)
                && route_run.current_segment >= route_run.job.distance
            {
                self.complete_route();
            }
        }
    }

    fn resolve_route_segment(&mut self) {
        let Some(route_run) = &mut self.route_run else {
            return;
        };
        let Some(resolution) = route_run.resolve_segment(&mut self.garage) else {
            self.complete_route();
            return;
        };

        let success = !matches!(
            resolution.outcome,
            SegmentOutcome::Crash | SegmentOutcome::CaughtByCops
        );
        self.pending_roll = Some(PendingRoll {
            label: format!(
                "route check {}/{}",
                resolution.segment_index + 1,
                resolution.segment_count
            ),
            final_value: resolution.roll,
            success,
            ticks: 0,
            frame_value: self.route_roll_frame,
            result: PendingRollResult::Segment(resolution),
        });
    }

    fn apply_segment_resolution(&mut self, resolution: SegmentResolution) {
        self.route_roll_frame = resolution.roll;
        self.last_resolution = None;
        match resolution.outcome {
            SegmentOutcome::Clear => {
                if let Some(outcome) = self
                    .segment_outcomes
                    .get_mut(resolution.segment_index as usize)
                {
                    *outcome = Some(true);
                }
                self.active_encounter_segment = None;
            }
            SegmentOutcome::Encounter(_) => {
                self.active_encounter_segment = Some(resolution.segment_index);
            }
            SegmentOutcome::Crash | SegmentOutcome::CaughtByCops => {
                if let Some(outcome) = self
                    .segment_outcomes
                    .get_mut(resolution.segment_index as usize)
                {
                    *outcome = Some(false);
                }
                self.active_encounter_segment = None;
            }
        }
        self.status = match resolution.outcome {
            SegmentOutcome::Clear => format!(
                "Segment {}/{} clear.",
                resolution.segment_index + 1,
                resolution.segment_count
            ),
            SegmentOutcome::Encounter(kind) => format!(
                "Segment {}/{} triggered a {:?} encounter.",
                resolution.segment_index + 1,
                resolution.segment_count,
                kind
            ),
            SegmentOutcome::Crash => "Critical failure: the car wrecked.".to_string(),
            SegmentOutcome::CaughtByCops => "Critical failure: caught by the cops.".to_string(),
        };
        self.last_segment_resolution = Some(resolution);

        let should_complete = self
            .route_run
            .as_ref()
            .map(|run| {
                run.car_wrecked
                    || run.caught_by_cops
                    || (run.phase == RoutePhase::Segment && run.current_segment >= run.job.distance)
            })
            .unwrap_or(false);
        if should_complete {
            self.complete_route();
        }
    }

    fn complete_route(&mut self) {
        let Some(route_run) = &mut self.route_run else {
            return;
        };
        let Some(report) = route_run.complete(&mut self.garage) else {
            return;
        };
        self.status = if report.car_wrecked {
            "Run failed: car wrecked and removed from the garage.".to_string()
        } else if report.caught_by_cops {
            "Run failed: caught by the cops and the car was impounded.".to_string()
        } else {
            format!(
                "Run complete: payout ${}, heat +{}, loot {}.",
                report.outcome.payout,
                report.outcome.heat_gained,
                report.loot.len()
            )
        };
        if report.car_wrecked || report.caught_by_cops {
            self.last_car_index = None;
            self.selected_car = self
                .selected_car
                .min(self.garage.cars.len().saturating_sub(1));
        }
        self.garage.refresh_black_market();
        self.garage.refresh_car_market();
        self.selected_market_part = self
            .selected_market_part
            .min(self.garage.black_market.len().saturating_sub(1));
        self.selected_market_car = self
            .selected_market_car
            .min(self.garage.car_market.len().saturating_sub(1));
        self.last_report = Some(report);
        self.route_run = None;
        self.last_segment_resolution = None;
        self.mode = Mode::PostRun;
        self.selected_action = 0;
    }

    fn apply_post_action(&mut self) {
        let actions = self.post_actions();
        let Some(action) = actions.get(self.selected_action).copied() else {
            return;
        };

        match action {
            PostAction::RepairCar => {
                let Some(car_index) = self.last_car_index else {
                    return;
                };
                let car_name = self.garage.cars[car_index].name.clone();
                if self.garage.repair_car(car_index) {
                    self.status = format!("Repaired {car_name}.");
                } else {
                    self.status = format!("{car_name} cannot be repaired right now.");
                }
            }
            PostAction::Continue => {
                self.last_report = None;
                self.last_resolution = None;
                self.mode = Mode::Garage;
                self.selected_action = 0;
                self.status = "Back at the garage.".to_string();
            }
        }
    }

    fn repair_selected_car_from_garage(&mut self) {
        if self.mode != Mode::Garage {
            return;
        }
        let car_name = self.garage.cars[self.selected_car].name.clone();
        if self.garage.repair_car(self.selected_car) {
            self.status = format!("Repaired {car_name}.");
        } else {
            self.status = format!("{car_name} cannot be repaired right now.");
        }
    }

    fn buy_selected_black_market_part(&mut self) {
        let Some(offer) = self
            .garage
            .black_market
            .get(self.selected_market_part)
            .cloned()
        else {
            self.status = "No black market part selected.".to_string();
            return;
        };
        if self.garage.cash < offer.asking_price {
            self.status = format!("{} costs ${}.", offer.item.name, offer.asking_price);
            return;
        }

        if self.garage.buy_black_market_part(self.selected_market_part) {
            self.selected_market_part = self
                .selected_market_part
                .min(self.garage.black_market.len().saturating_sub(1));
            self.selected_inventory = self.garage.inventory.len().saturating_sub(1);
            self.status = format!(
                "Bought {} for ${}. Install it from The Mechanic.",
                offer.item.name, offer.asking_price
            );
        } else {
            self.status = format!("Could not buy {}.", offer.item.name);
        }
    }

    fn sell_selected_black_market_item(&mut self) {
        let Some(item) = self.garage.inventory.get(self.selected_inventory).cloned() else {
            self.status = "No inventory item selected to sell.".to_string();
            return;
        };

        if let Some(sold_for) = self.garage.sell_inventory_item(self.selected_inventory) {
            self.selected_inventory = self
                .selected_inventory
                .min(self.garage.inventory.len().saturating_sub(1));
            self.status = format!("Sold {} for ${sold_for}.", item.name);
        } else {
            self.status = format!("Could not sell {}.", item.name);
        }
    }

    fn buy_selected_market_car(&mut self) {
        let Some(offer) = self
            .garage
            .car_market
            .get(self.selected_market_car)
            .cloned()
        else {
            self.status = "No market car selected.".to_string();
            return;
        };
        if self.garage.cash < offer.asking_price {
            self.status = format!("{} costs ${}.", offer.car.name, offer.asking_price);
            return;
        }

        if self.garage.buy_market_car(self.selected_market_car) {
            self.selected_market_car = self
                .selected_market_car
                .min(self.garage.car_market.len().saturating_sub(1));
            self.selected_car = self.garage.cars.len().saturating_sub(1);
            self.status = format!("Bought {} for ${}.", offer.car.name, offer.asking_price);
        } else {
            self.status = format!("Could not buy {}.", offer.car.name);
        }
    }

    fn sell_selected_market_car(&mut self) {
        let Some(car) = self.garage.cars.get(self.selected_car).cloned() else {
            self.status = "No owned car selected to sell.".to_string();
            return;
        };

        if let Some(sold_for) = self.garage.sell_car(self.selected_car) {
            self.selected_car = self
                .selected_car
                .min(self.garage.cars.len().saturating_sub(1));
            self.status = format!("Sold {} for ${sold_for}.", car.name);
        } else {
            self.status = format!("Cannot sell {}.", car.name);
        }
    }

    fn install_selected_inventory_item(&mut self) {
        let upgrades = self.vehicle_upgrade_inventory_items();
        if upgrades.is_empty() {
            self.status = "No vehicle upgrades available.".to_string();
            return;
        }
        let Some((inventory_index, item)) = upgrades.get(self.selected_mechanic_part) else {
            return;
        };
        if !is_installable(item) {
            self.status = format!("{} is sell-only.", item.name);
            return;
        }
        if let Some(cost) = install_cost(item) {
            if self.garage.cash < cost {
                self.status = format!("{} needs ${cost} to install.", item.name);
                return;
            }
        }
        let inventory_index = *inventory_index;
        let item = (*item).clone();
        let car_name = self.garage.cars[self.selected_car].name.clone();
        if self
            .garage
            .install_inventory_item(inventory_index, self.selected_car)
        {
            let cost = install_cost(&item).unwrap_or(0);
            self.status = format!("Installed {} on {car_name} for ${cost}.", item.name);
        } else {
            self.status = format!("{} cannot be installed on {car_name}.", item.name);
        }
        self.selected_mechanic_part = self.selected_mechanic_part.min(
            self.vehicle_upgrade_inventory_items()
                .len()
                .saturating_sub(1),
        );
    }

    fn uninstall_selected_upgrade_from_garage(&mut self) {
        if self.mode != Mode::Mechanic {
            return;
        }
        let Some(upgrade) = self.garage.cars[self.selected_car]
            .installed_upgrades
            .get(self.selected_upgrade)
            .cloned()
        else {
            self.status = "No installed mod selected.".to_string();
            return;
        };
        let car_name = self.garage.cars[self.selected_car].name.clone();
        if self
            .garage
            .uninstall_upgrade(self.selected_car, self.selected_upgrade)
        {
            self.selected_upgrade = self.selected_upgrade.min(
                self.garage.cars[self.selected_car]
                    .installed_upgrades
                    .len()
                    .saturating_sub(1),
            );
            self.status = format!("Uninstalled {} from {car_name}.", upgrade.name);
        } else {
            self.status = format!("{} cannot be uninstalled from {car_name}.", upgrade.name);
        }
    }

    fn route_action_count(&self) -> usize {
        let Some(route_run) = &self.route_run else {
            return 0;
        };
        match route_run.encounter_preview(&self.garage) {
            Some(RouteEncounterPreview::Patrol(previews)) => previews.len(),
            Some(RouteEncounterPreview::Fork(previews)) => previews.len(),
            None => 0,
        }
    }

    fn post_actions(&self) -> Vec<PostAction> {
        let mut actions = Vec::new();
        if let Some(car_index) = self.last_car_index {
            if self.garage.cars[car_index].repair_cost() > 0 {
                actions.push(PostAction::RepairCar);
            }
        }
        actions.push(PostAction::Continue);
        actions
    }

    fn inventory_value(&self) -> i32 {
        self.garage
            .inventory
            .iter()
            .map(|item| item.black_market_value)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingRollResult {
    Segment(SegmentResolution),
    Encounter(RouteResolution),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRoll {
    label: String,
    final_value: u8,
    success: bool,
    ticks: u8,
    frame_value: u8,
    result: PendingRollResult,
}

impl PendingRoll {
    fn progress_percent(&self) -> u8 {
        ((self.ticks.min(16) as u16 * 100) / 16) as u8
    }

    fn is_segment_roll(&self) -> bool {
        matches!(self.result, PendingRollResult::Segment(_))
    }

    fn is_encounter_roll(&self) -> bool {
        matches!(self.result, PendingRollResult::Encounter(_))
    }
}

fn styled_item(text: String, selected: bool) -> ListItem<'static> {
    let style = if selected {
        selected_row_style()
    } else {
        Style::default()
    };
    ListItem::new(text).style(style)
}

fn styled_line_item(line: Line<'static>, selected: bool) -> ListItem<'static> {
    let style = if selected {
        selected_row_style()
    } else {
        Style::default()
    };
    ListItem::new(line).style(style)
}

fn selected_row_style() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::Blue)
        .add_modifier(Modifier::BOLD)
}

fn styled_line(text: String, selected: bool) -> Line<'static> {
    let style = if selected {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    Line::from(Span::styled(text, style))
}

fn car_performance_lines(car: &Car) -> Vec<Line<'static>> {
    let effective = car.effective_stats();
    vec![
        Line::from(vec![
            Span::raw(format!("{} ", car.name)),
            rarity_badge(car.rarity, false),
        ]),
        compact_stat_line(
            "Cargo",
            car.cargo_capacity.to_string(),
            "Stealth",
            effective.stealth.to_string(),
        ),
        compact_stat_line(
            "Drive",
            format!("{:+}", effective.drivetrain),
            "Cond",
            format!("{}%", car.condition.average()),
        ),
        compact_stat_line(
            "Repair",
            format!("${}", car.repair_cost()),
            "Mods",
            car.installed_upgrades.len().to_string(),
        ),
        Line::from(""),
        Line::from(format!("Value ${} | {:?}", car.value, car.drivetrain)),
        Line::from(format!(
            "{} {} | {}",
            car.spec.engine.horsepower, "hp", car.spec.transmission
        )),
    ]
}

fn car_trade_lines(car: &Car) -> Vec<Line<'static>> {
    let mut lines = car_performance_lines(car);
    lines.push(Line::from(""));
    lines.push(subdued_line(format!(
        "{} {} | {}",
        car.spec.make, car.spec.model, car.spec.body_style
    )));
    lines
}

fn mechanic_part_item(marker: &str, item: &LootItem, selected: bool) -> ListItem<'static> {
    if !selected {
        return styled_line_item(
            Line::from(vec![
                Span::raw(format!("{marker} {} ", item.name)),
                rarity_badge(item.rarity, true),
                Span::raw(format!(" | ${}", item.black_market_value)),
            ]),
            false,
        );
    }

    let mut lines = vec![
        item_title_line(marker, &item.name, item.rarity),
        subdued_line(format!("Value: ${}", item.black_market_value)),
    ];
    append_loot_effect_lines(&mut lines, &item.effect);
    lines.push(Line::from(""));
    ListItem::new(lines)
}

fn mechanic_upgrade_item(
    marker: &str,
    upgrade: &InstalledUpgrade,
    selected: bool,
) -> ListItem<'static> {
    if !selected {
        return styled_line_item(
            Line::from(vec![
                Span::raw(format!("{marker} {} ", upgrade.name)),
                rarity_badge(upgrade.rarity, true),
            ]),
            false,
        );
    }

    let mut lines = vec![
        item_title_line(marker, &upgrade.name, upgrade.rarity),
        subdued_line(format!("Value: ${}", upgrade.black_market_value)),
    ];
    append_upgrade_effect_lines(&mut lines, upgrade.effect());
    lines.push(subdued_line(
        "Uninstall: returns part to inventory".to_string(),
    ));
    lines.push(Line::from(""));
    ListItem::new(lines)
}

fn title_line(text: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        text,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ))
}

fn compact_stat_line(
    left_label: &'static str,
    left_value: String,
    right_label: &'static str,
    right_value: String,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{left_label} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            left_value,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{right_label} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            right_value,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn wrap_index(current: usize, max: usize, delta: isize) -> usize {
    if delta < 0 {
        current.checked_sub(1).unwrap_or(max - 1)
    } else {
        (current + 1) % max
    }
}

fn preview_lines_for_patrol(
    index: usize,
    preview: &PatrolActionPreview,
    selected: bool,
) -> Vec<Line<'static>> {
    let marker = if selected { ">" } else { " " };
    if !selected {
        return vec![compact_patrol_line(marker, index + 1, preview)];
    }

    vec![
        choice_title_line(marker, index + 1, preview.action.label()),
        dice_line(format!("d100 <= {}", preview.chance)),
        outcome_line("Success", &preview.success, Color::Green),
        outcome_line("Failure", &preview.failure, Color::Red),
        subdued_line(format!("Check: {}", preview.check)),
        subdued_line(format!(
            "Modifiers: {}",
            format_modifiers(&preview.modifiers)
        )),
        Line::from(""),
    ]
}

fn preview_lines_for_fork(
    index: usize,
    preview: &ForkChoicePreview,
    selected: bool,
) -> Vec<Line<'static>> {
    let marker = if selected { ">" } else { " " };
    let roll = preview
        .chance
        .map(|chance| format!("d100 <= {chance}"))
        .unwrap_or_else(|| "none".to_string());
    if !selected {
        return vec![compact_fork_line(marker, index + 1, preview, &roll)];
    }

    vec![
        choice_title_line(marker, index + 1, preview.choice.label()),
        dice_line(roll),
        outcome_line("Success", &preview.success, Color::Green),
        outcome_line("Failure", &preview.failure, Color::Red),
        subdued_line(format!("Check: {}", preview.check)),
        subdued_line(format!(
            "Modifiers: {}",
            format_modifiers(&preview.modifiers)
        )),
        Line::from(""),
    ]
}

fn compact_patrol_line(marker: &str, index: usize, preview: &PatrolActionPreview) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{marker} {index}. {}  ", preview.action.label()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("d100 <= {}", preview.chance),
            Style::default().fg(Color::Cyan),
        ),
    ])
}

fn compact_fork_line(
    marker: &str,
    index: usize,
    preview: &ForkChoicePreview,
    roll: &str,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{marker} {index}. {}  ", preview.choice.label()),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(roll.to_string(), Style::default().fg(Color::Cyan)),
    ])
}

fn choice_title_line(marker: &str, index: usize, label: &str) -> Line<'static> {
    let style = if marker == ">" {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };
    let text = if index == 0 {
        format!("{marker} {label}")
    } else {
        format!("{marker} {index}. {label}")
    };
    Line::from(Span::styled(text, style))
}

fn item_title_line(marker: &str, name: &str, rarity: LootRarity) -> Line<'static> {
    let style = if marker == ">" {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    };
    Line::from(vec![
        Span::styled(format!("{marker} {name} "), style),
        rarity_badge(rarity, false),
    ])
}

fn dice_line(text: String) -> Line<'static> {
    Line::from(vec![
        Span::raw("   Roll: "),
        Span::styled(
            text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn route_track_line(
    current_segment: u8,
    segment_count: u8,
    width: u16,
    progress_percent: u8,
    segment_outcomes: &[Option<bool>],
    active_encounter_segment: Option<u8>,
) -> Line<'static> {
    let track_width = width
        .saturating_sub(6)
        .max(segment_count as u16 * 3)
        .max(12) as usize;
    let segment_count = segment_count.max(1) as usize;
    let current_segment = current_segment as usize;
    let mut spans = Vec::new();
    spans.push(Span::styled("   [", Style::default().fg(Color::DarkGray)));

    for index in 0..segment_count {
        let start = index * track_width / segment_count;
        let end = (index + 1) * track_width / segment_count;
        let segment_width = end.saturating_sub(start).max(1);
        if index == current_segment && progress_percent > 0 {
            let filled = ((segment_width as u16 * progress_percent.min(100) as u16) / 100)
                .min(segment_width as u16) as usize;
            let remaining = segment_width.saturating_sub(filled);
            if filled > 0 {
                spans.push(Span::styled(
                    "━".repeat(filled),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            if remaining > 0 {
                spans.push(Span::styled(
                    "╍".repeat(remaining),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            let outcome = segment_outcomes.get(index).copied().flatten();
            let is_active_encounter = active_encounter_segment == Some(index as u8);
            let glyph = if outcome.is_some() || is_active_encounter {
                "━"
            } else if index == current_segment {
                "╍"
            } else {
                "─"
            };
            let color = if let Some(success) = outcome {
                if success {
                    Color::Green
                } else {
                    Color::Red
                }
            } else if index == current_segment {
                Color::Yellow
            } else if is_active_encounter {
                Color::Yellow
            } else {
                Color::DarkGray
            };
            spans.push(Span::styled(
                glyph.repeat(segment_width),
                Style::default()
                    .fg(color)
                    .add_modifier(if index == current_segment {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ));
        }
    }

    spans.push(Span::styled("]", Style::default().fg(Color::DarkGray)));
    Line::from(spans)
}

fn subdued_line(text: String) -> Line<'static> {
    Line::from(Span::styled(
        format!("   {text}"),
        Style::default().fg(Color::DarkGray),
    ))
}

fn install_cost_line(cost: i32) -> Line<'static> {
    Line::from(vec![
        Span::styled("   Install cost: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("${cost}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn market_price_line(label: &'static str, price: i32) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("   {label}: "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("${price}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn market_price_line_into(lines: &mut Vec<Line<'static>>, label: &'static str, price: i32) {
    lines.push(market_price_line(label, price));
}

fn outcome_line(label: &'static str, text: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("   {label}: "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(text.to_string(), Style::default().fg(color)),
    ])
}

fn route_resolution_roll(resolution: &RouteResolution) -> Option<u8> {
    match resolution {
        RouteResolution::Patrol(resolution) => Some(resolution.roll),
        RouteResolution::Fork(resolution) => resolution.roll,
    }
}

fn route_resolution_success(resolution: &RouteResolution) -> bool {
    match resolution {
        RouteResolution::Patrol(resolution) => resolution.success,
        RouteResolution::Fork(resolution) => resolution.success,
    }
}

fn route_resolution_label(resolution: &RouteResolution) -> &'static str {
    match resolution {
        RouteResolution::Patrol(_) => "patrol check",
        RouteResolution::Fork(_) => "route choice",
    }
}

fn resolution_lines(resolution: &RouteResolution) -> Vec<Line<'static>> {
    match resolution {
        RouteResolution::Patrol(resolution) => patrol_resolution_lines(resolution),
        RouteResolution::Fork(resolution) => fork_resolution_lines(resolution),
    }
}

fn segment_resolution_lines(resolution: &SegmentResolution) -> Vec<Line<'static>> {
    let success = !matches!(
        resolution.outcome,
        SegmentOutcome::Crash | SegmentOutcome::CaughtByCops
    );
    let mut lines = vec![
        title_line("SEGMENT"),
        subdued_line(format!(
            "Encounter {}% | crash {}% | cops {}%",
            resolution.encounter_chance, resolution.crash_chance, resolution.caught_chance
        )),
        status_line(
            "Outcome",
            describe_segment_outcome(resolution.outcome),
            success,
        ),
    ];
    if matches!(resolution.outcome, SegmentOutcome::Crash) {
        negative_line(
            "Car wrecked and removed from garage".to_string(),
            &mut lines,
        );
    }
    if matches!(resolution.outcome, SegmentOutcome::CaughtByCops) {
        negative_line("Caught by cops; car impounded".to_string(), &mut lines);
    }
    for event in &resolution.events {
        match event {
            RouteEvent::SmoothSegment => lines.push(subdued_line(describe_event(event))),
            _ => negative_line(describe_event(event), &mut lines),
        }
    }
    lines
}

fn patrol_resolution_lines(resolution: &PatrolResolution) -> Vec<Line<'static>> {
    let mut lines = vec![
        title_line("PATROL"),
        subdued_line(format!("Target: {}", resolution.chance)),
        subdued_line(format!("Roll: {}", resolution.roll)),
        status_line(
            "Outcome",
            if resolution.success {
                "SUCCESS"
            } else {
                "FAILURE"
            },
            resolution.success,
        ),
    ];
    if !resolution.success {
        append_negative_consequences(&mut lines, resolution.heat_gained, &resolution.damage);
    }
    lines
}

fn fork_resolution_lines(resolution: &ForkResolution) -> Vec<Line<'static>> {
    let mut lines = vec![title_line("ROUTE CHOICE")];
    if let (Some(chance), Some(roll)) = (resolution.chance, resolution.roll) {
        lines.push(subdued_line(format!("Target: {chance}")));
        lines.push(subdued_line(format!("Roll: {roll}")));
        lines.push(status_line(
            "Outcome",
            if resolution.success {
                "SUCCESS"
            } else {
                "FAILURE"
            },
            resolution.success,
        ));
        if !resolution.success {
            append_negative_consequences(&mut lines, resolution.heat_gained, &resolution.damage);
        }
    } else {
        lines.push(status_line("Outcome", "SAFE ROUTE", true));
    }
    if resolution.payout_multiplier_percent < 100 {
        negative_line(
            format!(
                "Payout multiplier: {}%",
                resolution.payout_multiplier_percent
            ),
            &mut lines,
        );
    }
    lines
}

fn status_line(label: &'static str, text: &'static str, success: bool) -> Line<'static> {
    let color = if success { Color::Green } else { Color::Red };
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(Color::DarkGray)),
        Span::styled(
            text,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn negative_line(text: String, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(Span::styled(
        format!("! {text}"),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    )));
}

fn append_negative_consequences(
    lines: &mut Vec<Line<'static>>,
    heat_gained: u8,
    damage: &ddd_core::car::PartDamage,
) {
    if heat_gained > 0 {
        negative_line(format!("Heat gained: {heat_gained}"), lines);
    }
    if damage.any() {
        negative_line(
            format!(
                "Damage: engine -{}, suspension -{}, tires -{}, body -{}",
                damage.engine, damage.suspension, damage.tires, damage.body
            ),
            lines,
        );
    }
}

fn describe_segment_outcome(outcome: SegmentOutcome) -> &'static str {
    match outcome {
        SegmentOutcome::Clear => "CLEAR",
        SegmentOutcome::Encounter(_) => "ENCOUNTER",
        SegmentOutcome::Crash => "CRITICAL FAILURE",
        SegmentOutcome::CaughtByCops => "CAUGHT",
    }
}

fn format_modifiers(modifiers: &[ddd_core::CheckModifier]) -> String {
    if modifiers.is_empty() {
        return "none".to_string();
    }

    modifiers
        .iter()
        .map(|modifier| format!("{} {:+}", modifier.label, modifier.value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn post_action_label(action: PostAction) -> &'static str {
    match action {
        PostAction::RepairCar => "Repair dispatched car",
        PostAction::Continue => "Continue",
    }
}

fn is_installable(item: &LootItem) -> bool {
    is_vehicle_upgrade(item)
}

fn is_vehicle_upgrade(item: &LootItem) -> bool {
    matches!(item.effect, LootEffect::VehicleUpgrade(_))
}

fn install_cost(item: &LootItem) -> Option<i32> {
    match item.effect {
        LootEffect::SellOnly => None,
        LootEffect::VehicleUpgrade(effect) => Some(effect.install_cost),
        LootEffect::InstallCostVoucher(_) => None,
    }
}

fn append_loot_effect_lines(lines: &mut Vec<Line<'static>>, effect: &LootEffect) {
    match effect {
        LootEffect::SellOnly => lines.push(subdued_line("Use: sell-only".to_string())),
        LootEffect::VehicleUpgrade(effect) => append_upgrade_effect_lines(lines, *effect),
        LootEffect::InstallCostVoucher(effect) => lines.push(subdued_line(format!(
            "Use: -{}% off the next install cost",
            effect.discount_percent
        ))),
    }
}

fn append_upgrade_effect_lines(lines: &mut Vec<Line<'static>>, effect: UpgradeEffect) {
    if effect.stealth_modifier != 0 {
        lines.push(subdued_line(format!(
            "Stealth modifier: {:+}",
            effect.stealth_modifier
        )));
    }
    if effect.drivetrain_modifier != 0 {
        lines.push(subdued_line(format!(
            "Drivetrain modifier: {:+}",
            effect.drivetrain_modifier
        )));
    }
    if effect.repair_discount_percent > 0 {
        lines.push(subdued_line(format!(
            "Repair discount: -{}%",
            effect.repair_discount_percent
        )));
    }
    lines.push(install_cost_line(effect.install_cost));
}

fn describe_loot_effect(effect: &LootEffect) -> String {
    match effect {
        LootEffect::SellOnly => "sell-only".to_string(),
        LootEffect::VehicleUpgrade(effect) => describe_upgrade_effect(*effect),
        LootEffect::InstallCostVoucher(effect) => {
            format!("next install -{}%", effect.discount_percent)
        }
    }
}

fn loot_category_label(item: &LootItem) -> String {
    match item.effect {
        LootEffect::SellOnly => "sell-only".to_string(),
        LootEffect::VehicleUpgrade(_) => "vehicle upgrade".to_string(),
        LootEffect::InstallCostVoucher(effect) => {
            format!("install voucher -{}%", effect.discount_percent)
        }
    }
}

fn describe_upgrade_effect(effect: UpgradeEffect) -> String {
    let mut parts = Vec::new();
    if effect.stealth_modifier != 0 {
        parts.push(format!("stealth {:+}", effect.stealth_modifier));
    }
    if effect.drivetrain_modifier != 0 {
        parts.push(format!("drive {:+}", effect.drivetrain_modifier));
    }
    if effect.repair_discount_percent > 0 {
        parts.push(format!("repair -{}%", effect.repair_discount_percent));
    }
    parts.push(format!("install ${}", effect.install_cost));
    parts.join(", ")
}

fn loot_summary_line(item: &LootItem) -> Line<'static> {
    let style = rarity_style(item.rarity);
    Line::from(vec![
        Span::raw("- "),
        Span::styled(item.name.clone(), style.add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        rarity_badge(item.rarity, false),
        Span::raw(format!(
            " | ${} | {}",
            item.black_market_value,
            describe_loot_effect(&item.effect)
        )),
    ])
}

fn rarity_badge(rarity: LootRarity, compact: bool) -> Span<'static> {
    let label = match (rarity, compact) {
        (LootRarity::Common, true) => "C",
        (LootRarity::Uncommon, true) => "U",
        (LootRarity::Rare, true) => "R",
        (LootRarity::Common, false) => "Common",
        (LootRarity::Uncommon, false) => "Uncommon",
        (LootRarity::Rare, false) => "Rare",
    };
    Span::styled(
        format!("[{label}]"),
        rarity_style(rarity).add_modifier(Modifier::BOLD),
    )
}

fn rarity_style(rarity: LootRarity) -> Style {
    match rarity {
        LootRarity::Common => Style::default().fg(Color::White),
        LootRarity::Uncommon => Style::default().fg(Color::Green),
        LootRarity::Rare => Style::default().fg(Color::Magenta),
    }
}

fn describe_event(event: &RouteEvent) -> String {
    match event {
        RouteEvent::SmoothSegment => "clean segment".to_string(),
        RouteEvent::RoughRoad { damage } => format!(
            "rough road damage: engine -{}, suspension -{}, tires -{}, body -{}",
            damage.engine, damage.suspension, damage.tires, damage.body
        ),
        RouteEvent::PatrolCloseCall => "patrol close call; heat increased".to_string(),
        RouteEvent::Backtracked => "backtracked through a safer route".to_string(),
        RouteEvent::CaughtByCops => "caught by cops; car impounded".to_string(),
        RouteEvent::CargoTooLarge => "cargo did not fit in the selected car".to_string(),
        RouteEvent::Breakdown => "car broke down before finishing the route".to_string(),
    }
}
