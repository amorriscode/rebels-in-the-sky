use super::button::Button;
use super::clickable_list::ClickableListState;
use super::constants::*;
use super::gif_map::GifMap;
use super::ui_callback::{CallbackRegistry, UiCallback};
use super::utils::{format_satoshi, hover_text_target};
use super::{
    constants::{UiKey, IMG_FRAME_WIDTH, LEFT_PANEL_WIDTH},
    traits::{Screen, SplitPanel},
    widgets::{default_block, render_player_description, selectable_list},
};
use crate::network::trade::Trade;
use crate::types::AppResult;
use crate::{
    types::{PlayerId, TeamId},
    world::{
        player::Player,
        skill::Rated,
        types::{PlayerLocation, TeamLocation},
        world::World,
    },
};
use core::fmt::Debug;
use crossterm::event::KeyCode;
use ratatui::layout::Margin;
use ratatui::{
    layout::{Constraint, Layout},
    prelude::Rect,
    widgets::Paragraph,
    Frame,
};
use std::{sync::Arc, sync::Mutex, vec};
use strum_macros::Display;

#[derive(Debug, Clone, Copy, Display, Default, PartialEq, Hash)]
pub enum PlayerView {
    #[default]
    All,
    FreePirates,
    Tradable,
    OwnTeam,
}

impl PlayerView {
    fn next(&self) -> Self {
        match self {
            PlayerView::All => PlayerView::FreePirates,
            PlayerView::FreePirates => PlayerView::Tradable,
            PlayerView::Tradable => PlayerView::OwnTeam,
            PlayerView::OwnTeam => PlayerView::All,
        }
    }

    fn rule(&self, player: &Player, world: &World) -> bool {
        let own_team = if let Ok(team) = world.get_own_team() {
            team
        } else {
            return false;
        };

        match self {
            PlayerView::All => true,
            PlayerView::FreePirates => {
                if player.team.is_some() {
                    return false;
                }

                let player_planet_id = match player.current_location {
                    PlayerLocation::OnPlanet { planet_id } => planet_id,
                    _ => panic!("Free pirate must be PlayerLocation::OnPlanet"),
                };

                let own_team_planet_id = match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => planet_id,
                    TeamLocation::Travelling { to, .. } => to,
                    TeamLocation::Exploring { around, .. } => around,
                    TeamLocation::OnSpaceAdventure { around, .. } => around,
                };

                player_planet_id == own_team_planet_id
            }
            PlayerView::Tradable => {
                let own_team_planet_id = match own_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => planet_id,
                    _ => return false,
                };

                if player.team.is_none() {
                    return false;
                }

                if player.team.unwrap() == own_team.id {
                    return false;
                }

                let try_player_team = world.get_team_or_err(player.team.unwrap());
                if try_player_team.is_err() {
                    return false;
                }

                let player_team = try_player_team.unwrap();
                let player_team_planet_id = match player_team.current_location {
                    TeamLocation::OnPlanet { planet_id } => planet_id,
                    _ => return false,
                };

                player_team_planet_id == own_team_planet_id
            }
            PlayerView::OwnTeam => player.team.is_some() && player.team.unwrap() == own_team.id,
        }
    }

    fn to_string(&self) -> String {
        match self {
            PlayerView::All => "All".to_string(),
            PlayerView::FreePirates => "Free pirates".to_string(),
            PlayerView::Tradable => "Open for trade".to_string(),
            PlayerView::OwnTeam => "Own team".to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayerListPanel {
    pub index: usize,
    pub locked_player_id: Option<PlayerId>,
    pub selected_player_id: PlayerId,
    pub selected_team_id: Option<TeamId>,
    pub all_players: Vec<PlayerId>,
    pub players: Vec<PlayerId>,
    own_team_id: TeamId,
    view: PlayerView,
    update_view: bool,
    tick: usize,
    callback_registry: Arc<Mutex<CallbackRegistry>>,
    gif_map: Arc<Mutex<GifMap>>,
}

impl PlayerListPanel {
    pub fn new(
        callback_registry: Arc<Mutex<CallbackRegistry>>,
        gif_map: Arc<Mutex<GifMap>>,
    ) -> Self {
        Self {
            callback_registry,
            gif_map,
            ..Default::default()
        }
    }

    fn build_left_panel(&mut self, frame: &mut Frame, world: &World, area: Rect) {
        let split = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

        let hover_text_target = hover_text_target(frame);

        let mut filter_all_button = Button::new(
            format!("View: {}", PlayerView::All.to_string()).into(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::All,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View all players.".into(), hover_text_target);

        let mut filter_free_pirates_button = Button::new(
            format!("View: {}", PlayerView::FreePirates.to_string()).into(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::FreePirates,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View free pirates.".into(), hover_text_target);

        let mut filter_tradable_button = Button::new(
            format!("View: {}", PlayerView::Tradable.to_string()).into(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::Tradable,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View pirates open for trade.".into(), hover_text_target);

        let mut filter_own_team_button = Button::new(
            format!("View: {}", PlayerView::OwnTeam.to_string()).into(),
            UiCallback::SetPlayerPanelView {
                view: PlayerView::OwnTeam,
            },
            Arc::clone(&self.callback_registry),
        )
        .set_hotkey(UiKey::CYCLE_VIEW)
        .set_hover_text("View your own team players.".into(), hover_text_target);
        match self.view {
            PlayerView::All => filter_all_button.disable(None),
            PlayerView::FreePirates => filter_free_pirates_button.disable(None),
            PlayerView::Tradable => filter_tradable_button.disable(None),
            PlayerView::OwnTeam => filter_own_team_button.disable(None),
        }

        frame.render_widget(filter_all_button, split[0]);
        frame.render_widget(filter_free_pirates_button, split[1]);
        frame.render_widget(filter_tradable_button, split[2]);
        frame.render_widget(filter_own_team_button, split[3]);

        if self.players.len() > 0 {
            let mut options = vec![];
            for &player_id in self.players.iter() {
                let player = world.get_player(player_id);
                if player.is_none() {
                    continue;
                }
                let player = player.unwrap();
                let mut style = UiStyle::DEFAULT;
                if player.team.is_some() && player.team.unwrap() == world.own_team_id {
                    style = UiStyle::OK;
                } else if player.peer_id.is_some() {
                    style = UiStyle::NETWORK;
                }
                let full_name = player.info.full_name();
                let name = if full_name.len() <= 2 * MAX_NAME_LENGTH + 2 {
                    full_name
                } else {
                    player.info.shortened_name()
                };

                let text = format!("{:<26} {}", name, player.stars());
                options.push((text, style));
            }
            let list = selectable_list(options, &self.callback_registry);
            frame.render_stateful_widget(
                list.block(default_block().title("Players ↓/↑")),
                split[4],
                &mut ClickableListState::default().with_selected(Some(self.index)),
            );
        } else {
            frame.render_widget(default_block().title("Players"), split[4]);
        }
    }

    fn build_right_panel(&self, frame: &mut Frame, world: &World, area: Rect) -> AppResult<()> {
        let v_split = Layout::vertical([Constraint::Length(24), Constraint::Min(1)]).split(area);

        let h_split = Layout::horizontal([
            Constraint::Length(60),
            Constraint::Length(60),
            Constraint::Min(1),
        ])
        .split(v_split[0]);

        let button_split = Layout::horizontal([
            Constraint::Length(60),
            Constraint::Length(60),
            Constraint::Min(1),
        ])
        .split(v_split[1]);

        let player = world.get_player_or_err(self.selected_player_id)?;
        let own_team = world.get_own_team()?;

        // Display open trade if the selected and lock player are the two being traded.
        let mut open_trade = None;

        if let Some(locked_player_id) = self.locked_player_id {
            // First option: selected player is in own_team and locked player has a team
            // and this team has sent an offer containing exactly these players.
            if own_team.player_ids.contains(&player.id) {
                if let Some(trade) = own_team.received_trades.get(&(locked_player_id, player.id)) {
                    open_trade = Some(trade);
                }
            }
            // Second option: locked player is in own_team and selected player has a team
            // and this team has sent an offer containing exactly these players.
            if own_team.player_ids.contains(&locked_player_id) {
                if let Some(trade) = own_team.received_trades.get(&(player.id, locked_player_id)) {
                    open_trade = Some(trade);
                }
            }
        }

        render_player_description(
            player,
            &self.gif_map,
            &self.callback_registry,
            self.tick,
            frame,
            world,
            h_split[0],
        );
        self.render_buttons(
            player,
            open_trade,
            frame,
            world,
            button_split[0].inner(Margin {
                horizontal: 1,
                vertical: 0,
            }),
        )?;

        // If there is an open trade for the locked and selected players,
        // display a button to accept

        if let Some(locked_player_id) = self.locked_player_id {
            let locked_player = world.get_player_or_err(locked_player_id)?;
            render_player_description(
                locked_player,
                &self.gif_map,
                &self.callback_registry,
                self.tick,
                frame,
                world,
                h_split[1],
            );
            self.render_buttons(
                locked_player,
                open_trade,
                frame,
                world,
                button_split[1].inner(Margin {
                    horizontal: 1,
                    vertical: 0,
                }),
            )?;
        }

        Ok(())
    }

    fn render_buttons(
        &self,
        player: &Player,
        open_trade: Option<&Trade>,
        frame: &mut Frame,
        world: &World,
        area: Rect,
    ) -> AppResult<()> {
        let own_team = world.get_own_team()?;

        let buttons_split = Layout::vertical([
            Constraint::Length(3), //team
            Constraint::Length(3), //Lock/Unlock
            Constraint::Length(3), //hire info for FA or optionally trade
            Constraint::Min(1),
        ])
        .split(area);

        let hover_text_target = hover_text_target(frame);

        match player.current_location {
            PlayerLocation::OnPlanet { planet_id } => {
                let planet = world.get_planet_or_err(planet_id)?;
                let button = Button::new(
                    format!("Free pirate - On planet {}", planet.name).into(),
                    UiCallback::GoToPlanetZoomIn { planet_id },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Go to {}, the free agent's current location", planet.name),
                    hover_text_target,
                )
                .set_hotkey(UiKey::GO_TO_PLANET);
                frame.render_widget(button, buttons_split[0]);
            }
            PlayerLocation::WithTeam => {
                let team = world.get_team_or_err(player.team.unwrap())?;
                let button = Button::new(
                    format!("team {}", team.name).into(),
                    UiCallback::GoToPlayerTeam {
                        player_id: player.id,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(format!("Go to team {}", team.name), hover_text_target)
                .set_hotkey(UiKey::GO_TO_TEAM_ALTERNATIVE);
                frame.render_widget(button, buttons_split[0]);
            }
        }
        let lock_button =
            if self.locked_player_id.is_some() && self.locked_player_id.unwrap() == player.id {
                Button::new(
                    "Unlock".into(),
                    UiCallback::LockPlayerPanel {
                        player_id: player.id,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Unlock the player panel to allow browsing other players"),
                    hover_text_target,
                )
                .set_hotkey(UiKey::UNLOCK_PLAYER)
            } else {
                Button::new(
                    "Lock".into(),
                    UiCallback::LockPlayerPanel {
                        player_id: self.selected_player_id,
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!("Lock the player panel to keep the info while browsing"),
                    hover_text_target,
                )
                .set_hotkey(UiKey::LOCK_PLAYER)
            };
        frame.render_widget(lock_button, buttons_split[1]);

        // Add hire button for free pirates
        if player.team.is_none() {
            let can_hire = own_team.can_hire_player(&player);
            let hire_cost = player.hire_cost(own_team.reputation);

            let mut button = Button::new(
                format!("Hire -{}", format_satoshi(hire_cost)).into(),
                UiCallback::HirePlayer {
                    player_id: player.id,
                },
                Arc::clone(&self.callback_registry),
            )
            .set_hover_text(
                format!("Hire the free agent for {}", format_satoshi(hire_cost)),
                hover_text_target,
            )
            .set_hotkey(UiKey::HIRE);
            if can_hire.is_err() {
                button.disable(Some(format!("{}", can_hire.unwrap_err().to_string())));
            }

            frame.render_widget(button, buttons_split[2]);
        }
        // or if a trade exists and player is part of it, add trade buttons
        else if let Some(trade) = open_trade {
            let proposer_player = &trade.proposer_player;
            let target_player = &trade.target_player;
            if player.id == self.selected_player_id {
                let proposer_team = world
                    .get_team_or_err(proposer_player.team.expect("Player should have a team"))?;
                let mut button = Button::new(
                    "Accept trade".into(),
                    UiCallback::AcceptTrade {
                        trade: trade.clone(),
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!(
                        "Accept to trade {} for {}",
                        target_player.info.shortened_name(),
                        proposer_player.info.shortened_name(),
                    ),
                    hover_text_target,
                )
                .set_hotkey(UiKey::ACCEPT_TRADE);

                let can_trade =
                    proposer_team.can_trade_players(proposer_player, target_player, own_team);

                if can_trade.is_err() {
                    button.disable(Some(format!("{}", can_trade.unwrap_err().to_string())));
                }
                frame.render_widget(button, buttons_split[2]);
            } else if player.id == self.locked_player_id.expect("One player should be locked") {
                let button = Button::new(
                    "Decline trade".into(),
                    UiCallback::DeclineTrade {
                        trade: trade.clone(),
                    },
                    Arc::clone(&self.callback_registry),
                )
                .set_hover_text(
                    format!(
                        "Decline to trade {} for {}",
                        target_player.info.shortened_name(),
                        proposer_player.info.shortened_name(),
                    ),
                    hover_text_target,
                )
                .set_hotkey(UiKey::DECLINE_TRADE);

                frame.render_widget(button, buttons_split[2]);
            };
        }
        // or finally if either the selected or locked player are part of own_team (but not both)
        // add button to propose a trade.
        else if let Some(locked_player_id) = self.locked_player_id {
            //If player is selected and part of own team
            if own_team.player_ids.contains(&player.id) && player.id == self.selected_player_id {
                let proposer_player = world.get_player_or_err(player.id)?;
                let target_player = world.get_player_or_err(locked_player_id)?;
                if let Some(target_team_id) = target_player.team {
                    let target_team = world.get_team_or_err(target_team_id)?;
                    if own_team
                        .can_trade_players(proposer_player, target_player, target_team)
                        .is_ok()
                    {
                        let mut trade_button = Button::new(
                            "Propose trade".into(),
                            UiCallback::CreateTradeProposal {
                                proposer_player_id: proposer_player.id,
                                target_player_id: target_player.id,
                            },
                            Arc::clone(&self.callback_registry),
                        )
                        .set_hover_text(
                            format!(
                                "Propose to trade {} for {}",
                                proposer_player.info.shortened_name(),
                                target_player.info.shortened_name(),
                            ),
                            hover_text_target,
                        )
                        .set_hotkey(UiKey::CREATE_TRADE);

                        if own_team
                            .sent_trades
                            .get(&(proposer_player.id, target_player.id))
                            .is_some()
                        {
                            trade_button.disable(Some("Trade already proposed".into()));
                        }

                        frame.render_widget(trade_button, buttons_split[2]);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn set_view(&mut self, filter: PlayerView) {
        self.view = filter;
        self.update_view = true;
    }

    pub fn reset_view(&mut self) {
        self.set_view(PlayerView::All);
    }
}

impl Screen for PlayerListPanel {
    fn update(&mut self, world: &World) -> AppResult<()> {
        self.tick += 1;
        self.own_team_id = world.own_team_id;
        if world.dirty_ui || self.all_players.len() != world.players.len() {
            self.all_players = world.players.keys().into_iter().cloned().collect();
            self.all_players.sort_by(|a, b| {
                let a = world.get_player(*a).unwrap();
                let b = world.get_player(*b).unwrap();
                if a.rating() == b.rating() {
                    b.average_skill()
                        .partial_cmp(&a.average_skill())
                        .expect("Skill value should exist")
                } else {
                    b.rating().cmp(&a.rating())
                }
            });
            self.update_view = true;
        }
        if self.update_view {
            self.players = self
                .all_players
                .iter()
                .filter(|&&player_id| {
                    let player = world.get_player(player_id).unwrap();
                    self.view.rule(player, &world)
                })
                .map(|&player_id| player_id)
                .collect();
            self.update_view = false;
        }

        if self.index >= self.players.len() && self.players.len() > 0 {
            self.set_index(self.players.len() - 1);
        }

        if self.index < self.players.len() && self.players.len() > 0 {
            self.selected_player_id = self.players[self.index];
            self.selected_team_id = world.get_player_or_err(self.selected_player_id)?.team;
        }
        Ok(())
    }
    fn render(
        &mut self,
        frame: &mut Frame,
        world: &World,
        area: Rect,
        _debug_view: bool,
    ) -> AppResult<()> {
        if self.all_players.len() == 0 {
            frame.render_widget(
                Paragraph::new(" No player yet!"),
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 1,
                }),
            );
            return Ok(());
        }

        self.callback_registry
            .lock()
            .unwrap()
            .register_mouse_callback(
                crossterm::event::MouseEventKind::ScrollDown,
                None,
                UiCallback::NextPanelIndex,
            );

        self.callback_registry
            .lock()
            .unwrap()
            .register_mouse_callback(
                crossterm::event::MouseEventKind::ScrollUp,
                None,
                UiCallback::PreviousPanelIndex,
            );

        // Split into left and right panels
        let left_right_split = Layout::horizontal([
            Constraint::Length(LEFT_PANEL_WIDTH),
            Constraint::Min(IMG_FRAME_WIDTH),
        ])
        .split(area);
        self.build_left_panel(frame, world, left_right_split[0]);
        self.build_right_panel(frame, world, left_right_split[1])?;
        Ok(())
    }

    fn handle_key_events(
        &mut self,
        key_event: crossterm::event::KeyEvent,
        _world: &World,
    ) -> Option<UiCallback> {
        match key_event.code {
            KeyCode::Up => self.next_index(),
            KeyCode::Down => self.previous_index(),
            UiKey::GO_TO_TEAM => {
                if let Some(_) = self.selected_team_id.clone() {
                    return Some(UiCallback::GoToPlayerTeam {
                        player_id: self.selected_player_id,
                    });
                }
            }
            UiKey::CYCLE_VIEW => {
                return Some(UiCallback::SetPlayerPanelView {
                    view: self.view.next(),
                });
            }

            _ => {}
        }
        None
    }
}

impl SplitPanel for PlayerListPanel {
    fn index(&self) -> usize {
        self.index
    }

    fn max_index(&self) -> usize {
        self.players.len()
    }

    fn set_index(&mut self, index: usize) {
        self.index = index;
    }
}
