use super::{
    button::Button,
    clickable_list::{ClickableList, ClickableListItem},
    constants::*,
    gif_map::GifMap,
    hover_text_line::HoverTextLine,
    hover_text_span::HoverTextSpan,
    traits::UiStyled,
    ui_callback::{CallbackRegistry, UiCallback},
    utils::{format_satoshi, hover_text_target},
};
use crate::{
    game_engine::constants::MIN_TIREDNESS_FOR_ROLL_DECLINE,
    image::{player::PLAYER_IMAGE_WIDTH, spaceship::SPACESHIP_IMAGE_WIDTH},
    types::*,
    world::{
        constants::*,
        player::{Player, Trait},
        position::{GamePosition, Position, MAX_POSITION},
        resources::Resource,
        skill::{GameSkill, Rated, SKILL_NAMES},
        spaceship::{SpaceshipUpgrade, SpaceshipUpgradeTarget},
        team::Team,
        types::TeamLocation,
        world::World,
    },
};
use anyhow::anyhow;
use crossterm::event::KeyCode;
use once_cell::sync::Lazy;
use ratatui::{
    prelude::*,
    text::Span,
    widgets::{Block, BorderType, Borders, List, Paragraph},
    Frame,
};
use std::sync::{Arc, Mutex};

pub const UP_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↑", UiStyle::HEADER));
pub const UP_RIGHT_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↗", UiStyle::OK));
pub const DOWN_ARROW_SPAN: Lazy<Span<'static>> = Lazy::new(|| Span::styled("↓", UiStyle::ERROR));
pub const DOWN_RIGHT_ARROW_SPAN: Lazy<Span<'static>> =
    Lazy::new(|| Span::styled("↘", UiStyle::WARNING));

pub const SWITCH_ARROW_SPAN: Lazy<Span<'static>> =
    Lazy::new(|| Span::styled("⇆", Style::default().fg(Color::Yellow)));

pub fn default_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
}

pub fn default_list() -> List<'static> {
    List::new::<Vec<String>>(vec![])
}

pub fn selectable_list<'a>(
    options: Vec<(String, Style)>,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
) -> ClickableList<'a> {
    let items: Vec<ClickableListItem> = options
        .iter()
        .enumerate()
        .map(|(_, content)| {
            ClickableListItem::new(Span::styled(format!(" {}", content.0), content.1))
        })
        .collect();

    ClickableList::new(items, Arc::clone(&callback_registry))
        .highlight_style(UiStyle::SELECTED)
        .hovering_style(UiStyle::HIGHLIGHT)
}

pub fn go_to_team_current_planet_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let go_to_team_current_planet_button = match team.current_location {
        TeamLocation::OnPlanet { planet_id } => Button::new(
            format!("On planet {}", world.get_planet_or_err(planet_id)?.name).into(),
            UiCallback::GoToCurrentTeamPlanet { team_id: team.id },
            Arc::clone(&callback_registry),
        )
        .set_hover_text(
            format!("Go to planet {}", world.get_planet_or_err(planet_id)?.name),
            hover_text_target,
        )
        .set_hotkey(UiKey::GO_TO_PLANET),

        TeamLocation::Travelling {
            from: _from,
            to,
            started,
            duration,
            ..
        } => {
            let to = world.get_planet_or_err(to)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Travelling to {}", to).into()
            } else {
                "Landing".into()
            };

            let mut button = Button::new(text, UiCallback::None, Arc::clone(&callback_registry));
            button.disable(None);
            button.set_hover_text(format!("Travelling to planet {}", to), hover_text_target)
        }
        TeamLocation::Exploring {
            around,
            started,
            duration,
        } => {
            let around_planet = world.get_planet_or_err(around)?.name.to_string();
            let text = if started + duration > world.last_tick_short_interval + 3 * SECONDS {
                format!("Around {}", around_planet)
            } else {
                "Landing".into()
            };
            let countdown = if started + duration > world.last_tick_short_interval {
                (started + duration - world.last_tick_short_interval).formatted()
            } else {
                (0 as Tick).formatted()
            };
            let mut button = Button::new(
                format!("{} {}", text, countdown).into(),
                UiCallback::None,
                Arc::clone(&callback_registry),
            );
            button.disable(None);
            button.set_hover_text(
                format!("Exploring around planet {}", around_planet),
                hover_text_target,
            )
        }
        TeamLocation::OnSpaceAdventure { .. } => {
            return Err(anyhow!("Team is on a space adventure"))
        }
    };

    Ok(go_to_team_current_planet_button)
}

pub fn drink_button<'a>(
    world: &World,
    player_id: PlayerId,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let player = world.get_player_or_err(player_id)?;
    let can_drink = player.can_drink(world);

    let mut button = Button::new(
        "Drink!".into(),
        UiCallback::Drink { player_id },
        Arc::clone(&callback_registry),
    )
    .set_hotkey(UiKey::DRINK)
    .set_hover_text(
        "Drink a liter of rum, increasing morale and decreasing energy.".into(),
        hover_text_target,
    )
    .set_box_style(Resource::RUM.style());

    if can_drink.is_err() {
        button.disable(Some(format!("{}", can_drink.unwrap_err().to_string())));
    }

    Ok(button)
}

pub fn go_to_team_home_planet_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let planet_name = world.get_planet_or_err(team.home_planet_id)?.name.clone();
    Ok(Button::new(
        format!("Home planet: {planet_name}").into(),
        UiCallback::GoToHomePlanet { team_id: team.id },
        Arc::clone(&callback_registry),
    )
    .set_hover_text(
        format!("Go to team home planet {planet_name}",),
        hover_text_target,
    )
    .set_hotkey(UiKey::GO_TO_HOME_PLANET))
}

pub fn render_challenge_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
    hotkey: bool,
    frame: &mut Frame,
    area: Rect,
) -> AppResult<()> {
    let own_team = world.get_own_team()?;
    let can_challenge = own_team.can_challenge_team(team);

    if let Some(challenge) = own_team.received_challenges.get(&team.id) {
        let c_split = Layout::horizontal([
            Constraint::Min(10),
            Constraint::Length(6),
            Constraint::Length(6),
        ])
        .split(area);

        let accept_button = Button::new(
            format!("{:6^}", UiText::YES).into(),
            UiCallback::AcceptChallenge {
                challenge: challenge.clone(),
            },
            Arc::clone(&callback_registry),
        )
        .set_box_style(UiStyle::OK)
        .set_hover_text(
            format!("Accept the challenge from {} and start a game.", team.name),
            hover_text_target,
        );

        let decline_button = Button::new(
            format!("{:6^}", UiText::NO).into(),
            UiCallback::DeclineChallenge {
                challenge: challenge.clone(),
            },
            Arc::clone(&callback_registry),
        )
        .set_box_style(UiStyle::ERROR)
        .set_hover_text(
            format!("Decline the challenge from {}.", team.name),
            hover_text_target,
        );

        frame.render_widget(
            Paragraph::new("Challenged!")
                .centered()
                .block(default_block()),
            c_split[0],
        );

        frame.render_widget(accept_button, c_split[1]);
        frame.render_widget(decline_button, c_split[2]);
    } else {
        let challenge_button = if let Some(game_id) = team.current_game {
            // The game is not necessarily part of the world if it's a network game.
            let game_text = if let Ok(game) = world.get_game_or_err(game_id) {
                if let Some(action) = game.action_results.last() {
                    format!(
                        "{} {:>3}-{:<3} {}",
                        game.home_team_in_game.name,
                        action.home_score,
                        action.away_score,
                        game.away_team_in_game.name,
                    )
                } else {
                    format!(
                        "{}   0-0   {}",
                        game.home_team_in_game.name, game.away_team_in_game.name,
                    )
                }
            } else {
                "Unknown game".to_string()
            };
            Button::new(
                format!("Playing - {}", game_text).into(),
                UiCallback::GoToGame { game_id },
                Arc::clone(&callback_registry),
            )
            .set_hover_text("Go to team's game".into(), hover_text_target)
            .set_hotkey(UiKey::GO_TO_GAME)
        } else {
            let mut button = Button::new(
                "Challenge".into(),
                UiCallback::ChallengeTeam { team_id: team.id },
                Arc::clone(&callback_registry),
            )
            .set_hover_text(
                format!("Challenge {} to a game", team.name),
                hover_text_target,
            );

            if hotkey {
                button = button.set_hotkey(UiKey::CHALLENGE_TEAM)
            }

            if own_team.sent_challenges.get(&team.id).is_some() {
                button.disable(Some("Already challenged".into()));
            } else if can_challenge.is_err() {
                button.disable(Some(format!("{}", can_challenge.unwrap_err().to_string())));
            } else {
                button = if team.peer_id.is_some() {
                    button.set_box_style(UiStyle::NETWORK)
                } else {
                    button.set_box_style(UiStyle::OK)
                };
            }
            button
        };
        frame.render_widget(challenge_button, area)
    }

    Ok(())
}

pub fn trade_resource_button<'a>(
    world: &World,
    resource: Resource,
    amount: i32,
    unit_cost: u32,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
    hotkey: Option<KeyCode>,
    box_style: Style,
) -> AppResult<Button<'a>> {
    let mut button = Button::new(
        format!("{amount:^+}").into(),
        UiCallback::TradeResource {
            resource,
            amount,
            unit_cost,
        },
        Arc::clone(&callback_registry),
    )
    .set_box_style(box_style);

    if world
        .get_own_team()?
        .can_trade_resource(resource, amount, unit_cost)
        .is_err()
    {
        button.disable(None);
    }

    if amount == 0 {
        button.set_text("".into());
        button.disable(None);
    }

    let mut button = button.set_hover_text(
        format!(
            "{} {} {} for {}.",
            if amount > 0 { "Buy" } else { "Sell" },
            amount.abs(),
            resource,
            format_satoshi(amount.abs() as u32 * unit_cost),
        ),
        hover_text_target,
    );
    if let Some(key) = hotkey {
        button = button.set_hotkey(key);
    }

    Ok(button)
}

pub fn explore_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let duration = LONG_EXPLORATION_TIME;
    let mut button = Button::new(
        format!("Explore ({})", duration.formatted()).into(),
        UiCallback::ExploreAroundPlanet { duration },
        Arc::clone(&callback_registry),
    )
    .set_hotkey(UiKey::EXPLORE);

    match team.current_location {
        TeamLocation::OnPlanet { planet_id } => {
            let planet = world.get_planet_or_err(planet_id)?;
            let needed_fuel = (duration as f32 * team.spaceship_fuel_consumption()) as u32;
            button = button.set_hover_text(
                format!(
                    "Explore the space around {} on autopilot (need {} t of fuel). Hope to find resources, free pirates or more...",
                    planet.name,
                    needed_fuel
                ),
                hover_text_target,
            );

            if let Err(msg) = team.can_explore_around_planet(&planet, duration) {
                button.disable(Some(msg.to_string()));
            }
        }
        TeamLocation::Travelling {
            from: _from, to, ..
        } => {
            button = button.set_hover_text(
                "Explore the space on autopilot. Hope to find resources, free pirates or more..."
                    .to_string(),
                hover_text_target,
            );
            let to = world.get_planet_or_err(to)?.name.to_string();
            button.disable(Some(format!("Travelling to planet {}", to)));
        }
        TeamLocation::Exploring { around, .. } => {
            button = button.set_hover_text(
                "Explore the space on autopilot. Hope to find resources, free pirates or more..."
                    .to_string(),
                hover_text_target,
            );
            let around_planet = world.get_planet_or_err(around)?.name.to_string();
            button.disable(Some(format!("Exploring around planet {}", around_planet)));
        }
        TeamLocation::OnSpaceAdventure { .. } => {
            return Err(anyhow!("Team is on a space adventure"))
        }
    };

    Ok(button)
}

pub fn space_adventure_button<'a>(
    world: &World,
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
) -> AppResult<Button<'a>> {
    let mut button = Button::new(
        "Space Adventure".into(),
        UiCallback::StartSpaceAdventure,
        Arc::clone(&callback_registry),
    )
    .set_hotkey(UiKey::SPACE_ADVENTURE);

    match team.current_location {
        TeamLocation::OnPlanet { planet_id } => {
            let planet = world.get_planet_or_err(planet_id)?;
            button = button.set_hover_text(
                format!(
                    "Start a space adventure around {} to manually collect resources and more...",
                    planet.name,
                ),
                hover_text_target,
            );

            if let Err(msg) = team.can_start_space_adventure() {
                button.disable(Some(msg.to_string()));
            }
        }
        TeamLocation::Travelling {
            from: _from, to, ..
        } => {
            button = button.set_hover_text(
                format!("Start a space adventure to manually collect resources and more...",),
                hover_text_target,
            );
            let to = world.get_planet_or_err(to)?.name.to_string();
            button.disable(Some(format!("Travelling to planet {}", to)));
        }
        TeamLocation::Exploring { around, .. } => {
            button = button.set_hover_text(
                format!("Start a space adventure to manually collect resources and more...",),
                hover_text_target,
            );
            let around_planet = world.get_planet_or_err(around)?.name.to_string();
            button.disable(Some(format!("Exploring around planet {}", around_planet)));
        }
        TeamLocation::OnSpaceAdventure { .. } => {
            return Err(anyhow!("Already on a space adventure"))
        }
    };

    Ok(button)
}

pub(crate) fn get_storage_lengths(
    resources: &ResourceMap,
    storage_capacity: u32,
    bars_length: usize,
) -> Vec<usize> {
    let gold = resources.value(&Resource::GOLD);
    let scraps = resources.value(&Resource::SCRAPS);
    let rum = resources.value(&Resource::RUM);

    // Calculate temptative length
    let mut gold_length = ((Resource::GOLD.to_storing_space() * gold) as f32
        / storage_capacity as f32
        * bars_length as f32)
        .round() as usize;
    let mut scraps_length = ((Resource::SCRAPS.to_storing_space() * scraps) as f32
        / storage_capacity as f32
        * bars_length as f32)
        .round() as usize;
    let mut rum_length = ((Resource::RUM.to_storing_space() * rum) as f32 / storage_capacity as f32
        * bars_length as f32)
        .round() as usize;

    // If the quantity is larger than 0, we should display it with at least 1 bar.
    if gold > 0 {
        gold_length = gold_length.max(1);
    }
    if scraps > 0 {
        scraps_length = scraps_length.max(1);
    }
    if rum > 0 {
        rum_length = rum_length.max(1);
    }

    // free_bars can be negative because of the previous rule.
    let mut free_bars: isize =
        bars_length as isize - (gold_length + scraps_length + rum_length) as isize;

    // If free_bars is negative, remove enough bars from the largest length.
    if free_bars < 0 {
        if gold_length > scraps_length && gold_length > rum_length {
            gold_length -= (-free_bars) as usize;
        } else if rum_length > scraps_length {
            rum_length -= (-free_bars) as usize;
        } else {
            scraps_length -= (-free_bars) as usize;
        }
        free_bars = 0;
    } else if free_bars > 0 {
        // Round up to eliminate free bars when storage is full
        let free_space = storage_capacity - resources.used_storage_capacity();
        if free_space == 0 {
            if gold_length >= scraps_length && gold_length >= rum_length {
                gold_length += free_bars as usize;
            } else if rum_length >= scraps_length {
                rum_length += free_bars as usize;
            } else {
                scraps_length += free_bars as usize;
            }
            free_bars = 0
        }
    }

    vec![gold_length, scraps_length, rum_length, free_bars as usize]
}

pub fn upgrade_spaceship_button<'a>(
    team: &Team,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    hover_text_target: Rect,
    upgrade: SpaceshipUpgrade,
) -> AppResult<Button<'a>> {
    if team.spaceship.pending_upgrade.is_some() {
        return Err(anyhow!("Upgrading spaceship"));
    }

    let mut upgrade_button = Button::new(
        format!(
            "{} ({})",
            match upgrade.target {
                SpaceshipUpgradeTarget::Repairs { .. } => "Repair spaceship".to_string(),
                _ => format!("Upgrade {}", upgrade.target),
            },
            upgrade.duration.formatted()
        )
        .into(),
        UiCallback::SetUpgradeSpaceship {
            upgrade: upgrade.clone(),
        },
        Arc::clone(&callback_registry),
    )
    .set_hover_text(
        match upgrade.target {
            SpaceshipUpgradeTarget::Repairs { .. } => "Repair your spaceship.".to_string(),
            _ => "Upgrade your spaceship.".to_string(),
        },
        hover_text_target,
    )
    .set_hotkey(match upgrade.target {
        SpaceshipUpgradeTarget::Repairs { .. } => UiKey::REPAIR_SPACESHIP,
        _ => UiKey::UPGRADE_SPACESHIP,
    });

    let can_set_upgrade = team.can_set_upgrade_spaceship(upgrade);

    if can_set_upgrade.is_err() {
        upgrade_button.disable(Some(can_set_upgrade.unwrap_err().to_string()));
    }

    Ok(upgrade_button)
}

pub fn get_storage_spans(
    resources: &ResourceMap,
    storage_capacity: u32,
    bars_length: usize,
) -> Vec<Span> {
    if let [gold_length, scraps_length, rum_length, free_bars] =
        get_storage_lengths(resources, storage_capacity, bars_length)[..4]
    {
        vec![
            Span::raw(format!("Stiva: ",)),
            Span::styled("▰".repeat(gold_length), Resource::GOLD.style()),
            Span::styled("▰".repeat(scraps_length), Resource::SCRAPS.style()),
            Span::styled("▰".repeat(rum_length), Resource::RUM.style()),
            Span::raw("▱".repeat(free_bars)),
            Span::raw(format!(
                " {:>04}/{:<04} ",
                resources.used_storage_capacity(),
                storage_capacity
            )),
        ]
    } else {
        vec![Span::raw("")]
    }
}

pub fn get_crew_spans(team: &Team) -> Vec<Span> {
    let bars_length = team.spaceship.crew_capacity() as usize;
    let crew_length = team.player_ids.len();

    let crew_bars = format!(
        "{}{}",
        "▰".repeat(crew_length),
        "▱".repeat(bars_length - crew_length),
    );

    let crew_style = match crew_length {
        x if x < MIN_PLAYERS_PER_GAME => UiStyle::ERROR,
        x if x < team.spaceship.crew_capacity() as usize => UiStyle::WARNING,
        _ => UiStyle::OK,
    };

    vec![
        Span::raw(format!("Crew:  ")),
        Span::styled(crew_bars, crew_style),
        Span::raw(format!(
            " {}/{}  ",
            team.player_ids.len(),
            team.spaceship.crew_capacity()
        )),
    ]
}

pub fn get_durability_spans<'a>(value: u32, max_value: u32, bars_length: usize) -> Vec<Span<'a>> {
    let length = (value as f32 / max_value as f32 * bars_length as f32).round() as usize;
    let bars = format!("{}{}", "▰".repeat(length), "▱".repeat(bars_length - length),);

    let style = (20.0 * (value as f32 / max_value as f32)).bound().style();

    vec![
        Span::raw("Hull:  "),
        Span::styled(bars, style),
        Span::raw(format!(" {}/{}", value, max_value)),
    ]
}

pub fn get_charge_spans<'a>(
    value: u32,
    max_value: u32,
    is_recharging: bool,
    bars_length: usize,
) -> Vec<Span<'a>> {
    let length = (value as f32 / max_value as f32 * bars_length as f32).round() as usize;
    let bars = format!("{}{}", "▰".repeat(length), "▱".repeat(bars_length - length),);

    let style = if is_recharging {
        0.0.style()
    } else {
        (20.0 * (value as f32 / max_value as f32)).bound().style()
    };

    vec![
        Span::raw(if is_recharging {
            format!("{:>10}: ", "Recharging",)
        } else {
            format!("{:>10}: ", "Charge",)
        }),
        Span::styled(bars, style),
        Span::raw(format!(" {}/{}", value, max_value)),
    ]
}

pub fn get_fuel_spans<'a>(fuel: u32, fuel_capacity: u32, bars_length: usize) -> Vec<Span<'a>> {
    let fuel_length = (fuel as f32 / fuel_capacity as f32 * bars_length as f32).round() as usize;
    let fuel_bars = format!(
        "{}{}",
        "▰".repeat(fuel_length),
        "▱".repeat(bars_length - fuel_length),
    );

    let fuel_style = (20.0 * (fuel as f32 / fuel_capacity as f32))
        .bound()
        .style();

    vec![
        Span::raw(format!("Tank:  ",)),
        Span::styled(fuel_bars, fuel_style),
        Span::raw(format!(" {}/{}", fuel, fuel_capacity)),
    ]
}

pub fn render_spaceship_description(
    team: &Team,
    gif_map: &Arc<Mutex<GifMap>>,
    tick: usize,
    world: &World,
    frame: &mut Frame,
    area: Rect,
) {
    let spaceship_split = Layout::horizontal([
        Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2),
        Constraint::Min(1),
    ])
    .split(area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    }));

    if let Ok(lines) = gif_map
        .lock()
        .unwrap()
        .on_planet_spaceship_lines(&team.spaceship, tick)
    {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(
            paragraph.centered(),
            spaceship_split[0].inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );
    }

    let spaceship_info = if team.id == world.own_team_id {
        Paragraph::new(vec![
            Line::from(format!(
                "Speed: {:.3} AU/h",
                team.spaceship_speed() * HOURS as f32 / AU as f32
            )),
            Line::from(get_crew_spans(team)),
            Line::from(get_durability_spans(
                team.spaceship.current_durability(),
                team.spaceship.durability(),
                BARS_LENGTH,
            )),
            Line::from(get_fuel_spans(
                team.fuel(),
                team.spaceship.fuel_capacity(),
                BARS_LENGTH,
            )),
            Line::from(get_storage_spans(
                &team.resources,
                team.spaceship.storage_capacity(),
                BARS_LENGTH,
            )),
            Line::from(format!(
                "Consumption: {:.2} t/h",
                team.spaceship_fuel_consumption() * HOURS as f32
            )),
            Line::from(format!(
                "Max distance: {:.3} AU",
                team.spaceship.max_distance(team.fuel()) / AU as f32
            )),
            Line::from(format!(
                "Travelled: {:.3} AU",
                team.spaceship.total_travelled as f32 / AU as f32
            )),
            Line::from(format!("Value: {}", format_satoshi(team.spaceship.cost()),)),
        ])
    } else {
        Paragraph::new(vec![
            Line::from(format!(
                "Rating {}",
                world.team_rating(team.id).unwrap_or_default().stars()
            )),
            Line::from(format!("Reputation {}", team.reputation.stars())),
            Line::from(format!("Treasury: {}", format_satoshi(team.balance()))),
            Line::from(format!(
                "Game record: W{}/L{}/D{}",
                team.game_record[0], team.game_record[1], team.game_record[2]
            )),
            Line::from(get_crew_spans(team)),
        ])
    };

    frame.render_widget(
        spaceship_info,
        spaceship_split[1].inner(Margin {
            horizontal: 0,
            vertical: 1,
        }),
    );

    // Render main block
    let block = default_block().title(format!("Spaceship - {}", team.spaceship.name.to_string()));
    frame.render_widget(block, area);
}

pub fn render_spaceship_upgrade(
    team: &Team,
    upgrade: &SpaceshipUpgrade,
    gif_map: &Arc<Mutex<GifMap>>,
    tick: usize,
    frame: &mut Frame,
    area: Rect,
) {
    let spaceship_split = Layout::horizontal([
        Constraint::Length(SPACESHIP_IMAGE_WIDTH as u16 + 2),
        Constraint::Min(1),
    ])
    .split(area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    }));

    let mut upgraded_ship = team.spaceship.clone();

    match upgrade.target {
        SpaceshipUpgradeTarget::Hull { component } => upgraded_ship.hull = component.clone(),
        SpaceshipUpgradeTarget::Engine { component } => upgraded_ship.engine = component.clone(),
        SpaceshipUpgradeTarget::Storage { component } => upgraded_ship.storage = component.clone(),
        SpaceshipUpgradeTarget::Repairs { .. } => upgraded_ship.reset_durability(),
    }

    if let Ok(lines) = gif_map
        .lock()
        .unwrap()
        .in_shipyard_spaceship_lines(&upgraded_ship, tick)
    {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(
            paragraph.centered(),
            spaceship_split[0].inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        );
    }

    let storage_units = 0;
    let spaceship_info = Paragraph::new(vec![
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:.3}",
                "Max speed:",
                team.spaceship.speed(storage_units) * HOURS as f32 / AU as f32
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:.3}",
                    upgraded_ship.speed(storage_units) * HOURS as f32 / AU as f32
                ),
                if upgraded_ship.speed(storage_units) > team.spaceship.speed(storage_units) {
                    UiStyle::OK
                } else if upgraded_ship.speed(storage_units) < team.spaceship.speed(storage_units) {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" AU/h"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:<5}",
                "Max crew:",
                team.spaceship.crew_capacity()
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.crew_capacity()),
                if upgraded_ship.crew_capacity() > team.spaceship.crew_capacity() {
                    UiStyle::OK
                } else if upgraded_ship.crew_capacity() < team.spaceship.crew_capacity() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:<5}",
                "Max tank:",
                team.spaceship.fuel_capacity()
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.fuel_capacity()),
                if upgraded_ship.fuel_capacity() > team.spaceship.fuel_capacity() {
                    UiStyle::OK
                } else if upgraded_ship.fuel_capacity() < team.spaceship.fuel_capacity() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" t"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:<5}",
                "Max storage:",
                team.spaceship.storage_capacity(),
            )),
            Span::raw(" --> "),
            Span::styled(
                format!("{}", upgraded_ship.storage_capacity()),
                if upgraded_ship.storage_capacity() > team.spaceship.storage_capacity() {
                    UiStyle::OK
                } else if upgraded_ship.storage_capacity() < team.spaceship.storage_capacity() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:>2}/{:<2}",
                "Durability:",
                team.spaceship.current_durability(),
                team.spaceship.durability(),
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:>2}/{:<2}",
                    upgraded_ship.durability(),
                    upgraded_ship.durability()
                ),
                if upgraded_ship.durability() > team.spaceship.durability() {
                    UiStyle::OK
                } else if upgraded_ship.durability() < team.spaceship.durability() {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:.3}",
                "Consumption:",
                team.spaceship.fuel_consumption(storage_units) * HOURS as f32
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:.3}",
                    upgraded_ship.fuel_consumption(storage_units) * HOURS as f32
                ),
                if upgraded_ship.fuel_consumption(storage_units)
                    > team.spaceship.fuel_consumption(storage_units)
                {
                    UiStyle::OK
                } else if upgraded_ship.fuel_consumption(storage_units)
                    < team.spaceship.fuel_consumption(storage_units)
                {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" t/h"),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "{:<13} {:.3}",
                "Max distance:",
                team.spaceship.max_distance(team.spaceship.fuel_capacity()) / AU as f32
            )),
            Span::raw(" --> "),
            Span::styled(
                format!(
                    "{:.3}",
                    upgraded_ship.max_distance(upgraded_ship.fuel_capacity()) / AU as f32
                ),
                if upgraded_ship.max_distance(upgraded_ship.fuel_capacity())
                    > team.spaceship.max_distance(team.spaceship.fuel_capacity())
                {
                    UiStyle::OK
                } else if upgraded_ship.max_distance(upgraded_ship.fuel_capacity())
                    < team.spaceship.max_distance(team.spaceship.fuel_capacity())
                {
                    UiStyle::ERROR
                } else {
                    UiStyle::DEFAULT
                },
            ),
            Span::raw(" AU"),
        ]),
    ]);

    frame.render_widget(
        spaceship_info,
        spaceship_split[1].inner(Margin {
            horizontal: 0,
            vertical: 1,
        }),
    );
}

pub fn render_player_description(
    player: &Player,
    gif_map: &Arc<Mutex<GifMap>>,
    callback_registry: &Arc<Mutex<CallbackRegistry>>,
    tick: usize,
    frame: &mut Frame,
    world: &World,
    area: Rect,
) {
    let h_split = Layout::horizontal([
        Constraint::Length(PLAYER_IMAGE_WIDTH as u16 + 4),
        Constraint::Min(2),
    ])
    .split(area);

    let header_body_img = Layout::vertical([Constraint::Length(2), Constraint::Min(2)]).split(
        h_split[0].inner(Margin {
            horizontal: 2,
            vertical: 1,
        }),
    );

    let header_body_stats = Layout::vertical([
        Constraint::Length(2),  //margin
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //header
        Constraint::Length(1),  //margin
        Constraint::Length(20), //stats
    ])
    .split(h_split[1]);

    if let Ok(lines) = gif_map.lock().unwrap().player_frame_lines(&player, tick) {
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, header_body_img[1]);
    }

    let hover_text_target = hover_text_target(frame);

    let trait_span = if let Some(t) = player.special_trait {
        let trait_style = match t {
            Trait::Killer => UiStyle::TRAIT_KILLER,
            Trait::Relentless => UiStyle::TRAIT_RELENTLESS,
            Trait::Showpirate => UiStyle::TRAIT_SHOWPIRATE,
            Trait::Spugna => UiStyle::TRAIT_SPUGNA,
        };
        Span::styled(format!("{t}"), trait_style)
    } else {
        Span::raw("")
    };

    let line = HoverTextLine::from(vec![
        HoverTextSpan::new(
            Span::raw(format!(
                "Reputation {}  ",
                player.reputation.stars()
            )),
            format!("Reputation indicates how much the player is respected in the galaxy. It influences special bonuses and hiring cost. (current value {})", player.reputation.value()),
            hover_text_target,
            Arc::clone(&callback_registry),
        ),
        HoverTextSpan::new(
            trait_span,
            if let Some(t) = player.special_trait {
                t.description(&player)
            } else {
                    "".to_string()
            },
            hover_text_target,
            Arc::clone(&callback_registry),
        )
    ]);
    frame.render_widget(line, header_body_stats[1]);

    let mut morale = player.morale;
    // Check if player is currently playing.
    // In this case, read current morale from game.
    if let Some(team_id) = player.team {
        if let Ok(team) = world.get_team_or_err(team_id) {
            if let Some(game_id) = team.current_game {
                if let Ok(game) = world.get_game_or_err(game_id) {
                    if let Some(p) = if game.home_team_in_game.team_id == team_id {
                        game.home_team_in_game.players.get(&player.id)
                    } else {
                        game.away_team_in_game.players.get(&player.id)
                    } {
                        morale = p.morale;
                    }
                }
            }
        }
    }

    let morale_length = (morale / MAX_MORALE * BARS_LENGTH as f32).round() as usize;
    let morale_string = format!(
        "{}{}",
        "▰".repeat(morale_length),
        "▱".repeat(BARS_LENGTH - morale_length),
    );
    let morale_style = match morale {
        x if x > 1.75 * MORALE_THRESHOLD_FOR_LEAVING => UiStyle::OK,
        x if x > MORALE_THRESHOLD_FOR_LEAVING => UiStyle::WARNING,
        x if x > 0.0 => UiStyle::ERROR,
        _ => UiStyle::UNSELECTABLE,
    };

    frame.render_widget(
        HoverTextLine::from(vec![
            HoverTextSpan::new(
                Span::raw("Morale ".to_string()),
                format!(
                    "When morale is low, pirates may decide to leave the team! (current value {:.2})",
                    morale
                ),
                hover_text_target,
                Arc::clone(&callback_registry),
            ),
            HoverTextSpan::new(
                Span::styled(morale_string, morale_style),
                "",
                hover_text_target,
                Arc::clone(&callback_registry),
            ),
        ]),
        header_body_stats[2],
    );

    let mut tiredness = player.tiredness;
    // Check if player is currently playing.
    // In this case, read current tiredness from game.
    if let Some(team_id) = player.team {
        if let Ok(team) = world.get_team_or_err(team_id) {
            if let Some(game_id) = team.current_game {
                if let Ok(game) = world.get_game_or_err(game_id) {
                    if let Some(p) = if game.home_team_in_game.team_id == team_id {
                        game.home_team_in_game.players.get(&player.id)
                    } else {
                        game.away_team_in_game.players.get(&player.id)
                    } {
                        tiredness = p.tiredness;
                    }
                }
            }
        }
    }

    let tiredness_length = (tiredness / MAX_TIREDNESS * BARS_LENGTH as f32).round() as usize;
    let energy_string = format!(
        "{}{}",
        "▰".repeat(BARS_LENGTH - tiredness_length),
        "▱".repeat(tiredness_length),
    );
    let energy_style = match tiredness {
        x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 0.75 => UiStyle::OK,
        x if x < MIN_TIREDNESS_FOR_ROLL_DECLINE * 1.5 => UiStyle::WARNING,
        x if x < MAX_TIREDNESS => UiStyle::ERROR,
        _ => UiStyle::UNSELECTABLE,
    };

    frame.render_widget(
        HoverTextLine::from(vec![
            HoverTextSpan::new(
                Span::raw("Energy ".to_string()),
                format!("Energy affects player's performance in a game. When the energy goes to 0, the player is exhausted and will fail most game actions. (current value {:.2})", (MAX_TIREDNESS-tiredness)),
                hover_text_target,
                Arc::clone(&callback_registry)
            ),
            HoverTextSpan::new(Span::styled( energy_string, energy_style),"", hover_text_target,
            Arc::clone(&callback_registry)),
        ]),
        header_body_stats[3],
    );

    frame.render_widget(
        Paragraph::new(format!(
            "{} yo, {} cm, {} kg, {}",
            player.info.age as u8,
            player.info.height as u8,
            player.info.weight as u8,
            player.info.population,
        )),
        header_body_stats[4],
    );

    frame.render_widget(
        Paragraph::new(format_player_data(player)),
        header_body_stats[6],
    );

    // Render main block
    let block = default_block().title(format!(
        "{} {} {}",
        player.info.first_name,
        player.info.last_name,
        player.stars()
    ));
    frame.render_widget(block, area);
}

fn improvement_indicator<'a>(skill: f32, previous: f32) -> Span<'a> {
    // We only update at the end of the day, so we can display if something went recently up or not.
    if skill.value() > previous.value() {
        UP_ARROW_SPAN.clone()
    } else if skill > previous + 0.33 {
        UP_RIGHT_ARROW_SPAN.clone()
    } else if skill.value() < previous.value() {
        DOWN_ARROW_SPAN.clone()
    } else if skill < previous - 0.33 {
        DOWN_RIGHT_ARROW_SPAN.clone()
    } else {
        Span::styled(" ", UiStyle::DEFAULT)
    }
}

fn format_player_data(player: &Player) -> Vec<Line> {
    let skills = player.current_skill_array();
    let mut text = vec![];
    let mut roles = (0..MAX_POSITION)
        .map(|i: Position| {
            (
                i.as_str().to_string(),
                i.player_rating(player.current_skill_array()),
            )
        })
        .collect::<Vec<(String, f32)>>();
    roles.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut spans = vec![];
    spans.push(Span::styled(
        format!("{:<2} {:<5}          ", roles[0].0, roles[0].1.stars()),
        roles[0].1.style(),
    ));
    spans.push(Span::styled(
        format!("Athletics {:<5}", player.athletics.stars()),
        player.athletics.rating().style(),
    ));
    text.push(Line::from(spans));

    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<2} {:<5}       ", roles[i + 1].0, roles[i + 1].1.stars()),
            roles[i + 1].1.style(),
        ));

        spans.push(Span::styled(
            format!(
                "   {:<MAX_NAME_LENGTH$}{:02} ",
                SKILL_NAMES[i],
                skills[i].value(),
            ),
            skills[i].style(),
        ));
        spans.push(improvement_indicator(skills[i], player.previous_skills[i]));

        text.push(Line::from(spans));
    }
    text.push(Line::from(""));

    text.push(Line::from(vec![
        Span::styled(
            format!("{} {:<5}     ", "Offense", player.offense.stars()),
            player.offense.rating().style(),
        ),
        Span::styled(
            format!("{} {}", "Defense", player.defense.stars()),
            player.defense.rating().style(),
        ),
    ]));
    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<10}{:02} ", SKILL_NAMES[i + 4], skills[i + 4].value(),),
            skills[i + 4].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 4],
            player.previous_skills[i + 4],
        ));

        spans.push(Span::styled(
            format!(
                "    {:<MAX_NAME_LENGTH$}{:02} ",
                SKILL_NAMES[i + 8],
                skills[i + 8].value(),
            ),
            skills[i + 8].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 8],
            player.previous_skills[i + 8],
        ));

        text.push(Line::from(spans));
    }
    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled(
            format!("{} {:<5}   ", "Technical", player.technical.stars()),
            player.technical.rating().style(),
        ),
        Span::styled(
            format!("{} {}", "Mental", player.mental.stars()),
            player.mental.rating().style(),
        ),
    ]));

    for i in 0..4 {
        let mut spans = vec![];
        spans.push(Span::styled(
            format!("{:<10}{:02} ", SKILL_NAMES[i + 12], skills[i + 12].value(),),
            skills[i + 12].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 12],
            player.previous_skills[i + 12],
        ));

        spans.push(Span::styled(
            format!(
                "    {:<MAX_NAME_LENGTH$}{:02} ",
                SKILL_NAMES[i + 16],
                skills[i + 16].value(),
            ),
            skills[i + 16].style(),
        ));
        spans.push(improvement_indicator(
            skills[i + 16],
            player.previous_skills[i + 16],
        ));

        text.push(Line::from(spans));
    }

    text
}

#[cfg(test)]
mod tests {
    use crate::{
        types::{PlanetId, TeamId},
        ui::widgets::get_storage_lengths,
        world::{resources::Resource, spaceship::SpaceshipPrefab, team::Team},
    };

    use super::{AppResult, StorableResourceMap, BARS_LENGTH};

    #[test]
    fn test_storage_spans() -> AppResult<()> {
        let mut team = Team::random(
            TeamId::new_v4(),
            PlanetId::new_v4(),
            "test".into(),
            "test".into(),
        );
        team.spaceship = SpaceshipPrefab::Bresci.spaceship("test".into());

        let bars_length = BARS_LENGTH;
        if let [gold_length, scraps_length, rum_length, free_bars] = get_storage_lengths(
            &team.resources,
            team.spaceship.storage_capacity(),
            bars_length,
        )[..4]
        {
            println!("{:?}", team.resources);
            println!(
                "gold={} scraps={} rum={} free={} storage={}/{}",
                gold_length,
                scraps_length,
                rum_length,
                free_bars,
                team.used_storage_capacity(),
                team.storage_capacity()
            );
            assert_eq!(gold_length, 0);
            assert_eq!(scraps_length, 0);
            assert_eq!(rum_length, 0);
            assert_eq!(free_bars, bars_length);
            assert_eq!(
                gold_length + scraps_length + rum_length + free_bars,
                bars_length
            );
        } else {
            panic!("Failed to calculate resource length");
        }

        team.resources
            .add(Resource::SCRAPS, 178, team.storage_capacity())?;
        team.resources
            .add(Resource::RUM, 11, team.storage_capacity())?;

        if let [gold_length, scraps_length, rum_length, free_bars] = get_storage_lengths(
            &team.resources,
            team.spaceship.storage_capacity(),
            bars_length,
        )[..4]
        {
            println!("{:?}", team.resources);
            println!(
                "gold={} scraps={} rum={} free={} storage={}/{}",
                gold_length,
                scraps_length,
                rum_length,
                free_bars,
                team.used_storage_capacity(),
                team.storage_capacity()
            );
            assert_eq!(gold_length, 0);
            assert_eq!(scraps_length, 21);
            assert_eq!(rum_length, 1);
            assert_eq!(free_bars, 3);
            assert_eq!(
                gold_length + scraps_length + rum_length + free_bars,
                bars_length
            );
        } else {
            panic!("Failed to calculate resource length");
        }
        team.resources
            .add(Resource::SCRAPS, 24, team.storage_capacity())?;
        team.resources
            .add(Resource::GOLD, 1, team.storage_capacity())?;

        if let [gold_length, scraps_length, rum_length, free_bars] = get_storage_lengths(
            &team.resources,
            team.spaceship.storage_capacity(),
            bars_length,
        )[..4]
        {
            println!("{:?}", team.resources);
            println!(
                "gold={} scraps={} rum={} free={} storage={}/{}",
                gold_length,
                scraps_length,
                rum_length,
                free_bars,
                team.used_storage_capacity(),
                team.storage_capacity()
            );
            assert_eq!(gold_length, 1);
            assert_eq!(scraps_length, 23);
            assert_eq!(rum_length, 1);
            assert_eq!(free_bars, 0);
            assert_eq!(
                gold_length + scraps_length + rum_length + free_bars,
                bars_length
            );
        } else {
            panic!("Failed to calculate resource length");
        }

        Ok(())
    }
}
