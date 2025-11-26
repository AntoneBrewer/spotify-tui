use crate::app::{
  ActiveBlock, AlbumTableContext, App, Artist, ArtistBlock, EpisodeTableContext, RouteId,
  ScrollableResultPages, SelectedAlbum, SelectedFullAlbum, SelectedFullShow, SelectedShow,
  TrackTableContext,
};
use crate::config::ClientConfig;
use anyhow::anyhow;
use rspotify::{
  prelude::*,
  model::{
    album::SimplifiedAlbum,
    artist::FullArtist,
    page::Page,
    playlist::{PlaylistItem, SimplifiedPlaylist},
    recommend::Recommendations,
    search::SearchResult,
    show::SimplifiedShow,
    track::FullTrack,
    PlayableItem, AdditionalType, Country, Market, RepeatState, SearchType,
    AlbumId, ArtistId, TrackId, PlaylistId, ShowId, EpisodeId, PlayableId, PlayContextId,
    Offset,
  },
  AuthCodeSpotify, Credentials, OAuth, Token,
};
use std::{
  sync::Arc,
  time::{Duration, Instant},
};
use chrono::{DateTime, Utc, TimeDelta};
use tokio::sync::Mutex;
use tokio::try_join;

#[derive(Debug)]
pub enum IoEvent {
  GetCurrentPlayback,
  RefreshAuthentication,
  GetPlaylists,
  GetDevices,
  GetSearchResults(String, Option<Country>),
  SetTracksToTable(Vec<FullTrack>),
  GetMadeForYouPlaylistTracks(String, u32),
  GetPlaylistTracks(String, u32),
  GetCurrentSavedTracks(Option<u32>),
  StartPlayback(Option<String>, Option<Vec<String>>, Option<usize>),
  UpdateSearchLimits(u32, u32),
  Seek(u32),
  NextTrack,
  PreviousTrack,
  Shuffle(bool),
  Repeat(RepeatState),
  PausePlayback,
  ChangeVolume(u8),
  GetArtist(String, String, Option<Country>),
  GetAlbumTracks(Box<SimplifiedAlbum>),
  GetRecommendationsForSeed(
    Option<Vec<String>>,
    Option<Vec<String>>,
    Box<Option<FullTrack>>,
    Option<Country>,
  ),
  GetCurrentUserSavedAlbums(Option<u32>),
  CurrentUserSavedAlbumsContains(Vec<String>),
  CurrentUserSavedAlbumDelete(String),
  CurrentUserSavedAlbumAdd(String),
  UserUnfollowArtists(Vec<String>),
  UserFollowArtists(Vec<String>),
  UserFollowPlaylist(String, String, Option<bool>),
  UserUnfollowPlaylist(String, String),
  MadeForYouSearchAndAdd(String, Option<Country>),
  GetAudioAnalysis(String),
  GetUser,
  ToggleSaveTrack(String),
  GetRecommendationsForTrackId(String, Option<Country>),
  GetRecentlyPlayed,
  GetFollowedArtists(Option<String>),
  SetArtistsToTable(Vec<FullArtist>),
  UserArtistFollowCheck(Vec<String>),
  GetAlbum(String),
  TransferPlaybackToDevice(String),
  GetAlbumForTrack(String),
  CurrentUserSavedTracksContains(Vec<String>),
  GetCurrentUserSavedShows(Option<u32>),
  CurrentUserSavedShowsContains(Vec<String>),
  CurrentUserSavedShowDelete(String),
  CurrentUserSavedShowAdd(String),
  GetShowEpisodes(Box<SimplifiedShow>),
  GetShow(String),
  GetCurrentShowEpisodes(String, Option<u32>),
  AddItemToQueue(String),
}

pub fn get_spotify(token: Token) -> (AuthCodeSpotify, DateTime<Utc>) {
  let token_expiry = token.expires_at.unwrap_or(Utc::now() + TimeDelta::try_hours(1).unwrap());
  
  let creds = Credentials::default();
  let oauth = OAuth::default();
  
  let spotify = AuthCodeSpotify::new(creds, oauth);
  
  // Set the token
  let _ = spotify.token.lock().map(|mut guard| {
    *guard = Some(token);
  });
  
  (spotify, token_expiry)
}

#[derive(Clone)]
pub struct Network<'a> {
  creds: Credentials,
  oauth: OAuth,
  pub spotify: AuthCodeSpotify,
  large_search_limit: u32,
  small_search_limit: u32,
  pub client_config: ClientConfig,
  pub app: &'a Arc<Mutex<App>>,
}

impl<'a> Network<'a> {
  pub fn new(
    creds: Credentials,
    oauth: OAuth,
    spotify: AuthCodeSpotify,
    client_config: ClientConfig,
    app: &'a Arc<Mutex<App>>,
  ) -> Self {
    Network {
      creds,
      oauth,
      spotify,
      large_search_limit: 20,
      small_search_limit: 4,
      client_config,
      app,
    }
  }

  #[allow(clippy::cognitive_complexity)]
  pub async fn handle_network_event(&mut self, io_event: IoEvent) {
    match io_event {
      IoEvent::RefreshAuthentication => {
        self.refresh_authentication().await;
      }
      IoEvent::GetPlaylists => {
        self.get_current_user_playlists().await;
      }
      IoEvent::GetUser => {
        self.get_user().await;
      }
      IoEvent::GetDevices => {
        self.get_devices().await;
      }
      IoEvent::GetCurrentPlayback => {
        self.get_current_playback().await;
      }
      IoEvent::SetTracksToTable(full_tracks) => {
        self.set_tracks_to_table(full_tracks).await;
      }
      IoEvent::GetSearchResults(search_term, country) => {
        self.get_search_results(search_term, country).await;
      }
      IoEvent::GetMadeForYouPlaylistTracks(playlist_id, made_for_you_offset) => {
        self
          .get_made_for_you_playlist_tracks(playlist_id, made_for_you_offset)
          .await;
      }
      IoEvent::GetPlaylistTracks(playlist_id, playlist_offset) => {
        self.get_playlist_tracks(playlist_id, playlist_offset).await;
      }
      IoEvent::GetCurrentSavedTracks(offset) => {
        self.get_current_user_saved_tracks(offset).await;
      }
      IoEvent::StartPlayback(context_uri, uris, offset) => {
        self.start_playback(context_uri, uris, offset).await;
      }
      IoEvent::UpdateSearchLimits(large_search_limit, small_search_limit) => {
        self.large_search_limit = large_search_limit;
        self.small_search_limit = small_search_limit;
      }
      IoEvent::Seek(position_ms) => {
        self.seek(position_ms).await;
      }
      IoEvent::NextTrack => {
        self.next_track().await;
      }
      IoEvent::PreviousTrack => {
        self.previous_track().await;
      }
      IoEvent::Repeat(repeat_state) => {
        self.repeat(repeat_state).await;
      }
      IoEvent::PausePlayback => {
        self.pause_playback().await;
      }
      IoEvent::ChangeVolume(volume) => {
        self.change_volume(volume).await;
      }
      IoEvent::GetArtist(artist_id, input_artist_name, country) => {
        self.get_artist(artist_id, input_artist_name, country).await;
      }
      IoEvent::GetAlbumTracks(album) => {
        self.get_album_tracks(album).await;
      }
      IoEvent::GetRecommendationsForSeed(seed_artists, seed_tracks, first_track, country) => {
        self
          .get_recommendations_for_seed(seed_artists, seed_tracks, first_track, country)
          .await;
      }
      IoEvent::GetCurrentUserSavedAlbums(offset) => {
        self.get_current_user_saved_albums(offset).await;
      }
      IoEvent::CurrentUserSavedAlbumsContains(album_ids) => {
        self.current_user_saved_albums_contains(album_ids).await;
      }
      IoEvent::CurrentUserSavedAlbumDelete(album_id) => {
        self.current_user_saved_album_delete(album_id).await;
      }
      IoEvent::CurrentUserSavedAlbumAdd(album_id) => {
        self.current_user_saved_album_add(album_id).await;
      }
      IoEvent::UserUnfollowArtists(artist_ids) => {
        self.user_unfollow_artists(artist_ids).await;
      }
      IoEvent::UserFollowArtists(artist_ids) => {
        self.user_follow_artists(artist_ids).await;
      }
      IoEvent::UserFollowPlaylist(playlist_owner_id, playlist_id, is_public) => {
        self
          .user_follow_playlist(playlist_owner_id, playlist_id, is_public)
          .await;
      }
      IoEvent::UserUnfollowPlaylist(user_id, playlist_id) => {
        self.user_unfollow_playlist(user_id, playlist_id).await;
      }
      IoEvent::MadeForYouSearchAndAdd(search_term, country) => {
        self.made_for_you_search_and_add(search_term, country).await;
      }
      IoEvent::GetAudioAnalysis(uri) => {
        self.get_audio_analysis(uri).await;
      }
      IoEvent::ToggleSaveTrack(track_id) => {
        self.toggle_save_track(track_id).await;
      }
      IoEvent::GetRecommendationsForTrackId(track_id, country) => {
        self
          .get_recommendations_for_track_id(track_id, country)
          .await;
      }
      IoEvent::GetRecentlyPlayed => {
        self.get_recently_played().await;
      }
      IoEvent::GetFollowedArtists(after) => {
        self.get_followed_artists(after).await;
      }
      IoEvent::SetArtistsToTable(full_artists) => {
        self.set_artists_to_table(full_artists).await;
      }
      IoEvent::UserArtistFollowCheck(artist_ids) => {
        self.user_artist_check_follow(artist_ids).await;
      }
      IoEvent::GetAlbum(album_id) => {
        self.get_album(album_id).await;
      }
      IoEvent::TransferPlaybackToDevice(device_id) => {
        self.transfer_playback_to_device(device_id).await;
      }
      IoEvent::GetAlbumForTrack(track_id) => {
        self.get_album_for_track(track_id).await;
      }
      IoEvent::Shuffle(shuffle_state) => {
        self.shuffle(shuffle_state).await;
      }
      IoEvent::CurrentUserSavedTracksContains(track_ids) => {
        self.current_user_saved_tracks_contains(track_ids).await;
      }
      IoEvent::GetCurrentUserSavedShows(offset) => {
        self.get_current_user_saved_shows(offset).await;
      }
      IoEvent::CurrentUserSavedShowsContains(show_ids) => {
        self.current_user_saved_shows_contains(show_ids).await;
      }
      IoEvent::CurrentUserSavedShowDelete(show_id) => {
        self.current_user_saved_shows_delete(show_id).await;
      }
      IoEvent::CurrentUserSavedShowAdd(show_id) => {
        self.current_user_saved_shows_add(show_id).await;
      }
      IoEvent::GetShowEpisodes(show) => {
        self.get_show_episodes(show).await;
      }
      IoEvent::GetShow(show_id) => {
        self.get_show(show_id).await;
      }
      IoEvent::GetCurrentShowEpisodes(show_id, offset) => {
        self.get_current_show_episodes(show_id, offset).await;
      }
      IoEvent::AddItemToQueue(item) => {
        self.add_item_to_queue(item).await;
      }
    };

    let mut app = self.app.lock().await;
    app.is_loading = false;
  }

  async fn handle_error(&mut self, e: anyhow::Error) {
    let mut app = self.app.lock().await;
    app.handle_error(e);
  }

  async fn get_user(&mut self) {
    match self.spotify.current_user().await {
      Ok(user) => {
        let mut app = self.app.lock().await;
        app.user = Some(user);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_devices(&mut self) {
    if let Ok(result) = self.spotify.device().await {
      let mut app = self.app.lock().await;
      app.push_navigation_stack(RouteId::SelectedDevice, ActiveBlock::SelectDevice);
      if !result.is_empty() {
        app.devices = Some(rspotify::model::device::DevicePayload { devices: result });
        // Select the first device in the list
        app.selected_device_index = Some(0);
      }
    }
  }

  async fn get_current_playback(&mut self) {
    let context = self
      .spotify
      .current_playback(
        None::<Market>,
        Some(&[AdditionalType::Episode, AdditionalType::Track]),
      )
      .await;

    match context {
      Ok(Some(c)) => {
        let mut app = self.app.lock().await;
        app.current_playback_context = Some(c.clone());
        app.instant_since_last_current_playback_poll = Instant::now();

        if let Some(item) = c.item {
          match item {
            PlayableItem::Track(track) => {
              if let Some(track_id) = track.id {
                app.dispatch(IoEvent::CurrentUserSavedTracksContains(vec![track_id.id().to_string()]));
              };
            }
            PlayableItem::Episode(_episode) => { /*should map this to following the podcast show*/ }
          }
        };
      }
      Ok(None) => {
        let mut app = self.app.lock().await;
        app.instant_since_last_current_playback_poll = Instant::now();
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }

    let mut app = self.app.lock().await;
    app.seek_ms.take();
    app.is_fetching_current_playback = false;
  }

  async fn current_user_saved_tracks_contains(&mut self, ids: Vec<String>) {
    let track_ids: Vec<TrackId> = ids
      .iter()
      .filter_map(|id| TrackId::from_id(id).ok())
      .collect();
    
    match self.spotify.current_user_saved_tracks_contains(track_ids.iter().map(|t| t.as_ref())).await {
      Ok(is_saved_vec) => {
        let mut app = self.app.lock().await;
        for (i, id) in ids.iter().enumerate() {
          if let Some(is_liked) = is_saved_vec.get(i) {
            if *is_liked {
              app.liked_song_ids_set.insert(id.to_string());
            } else {
              if app.liked_song_ids_set.contains(id) {
                app.liked_song_ids_set.remove(id);
              }
            }
          };
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_playlist_tracks(&mut self, playlist_id: String, playlist_offset: u32) {
    if let Ok(playlist_id_obj) = PlaylistId::from_id(&playlist_id) {
      if let Ok(playlist_tracks) = self
        .spotify
        .playlist_items_manual(
          playlist_id_obj,
          None,
          None,
          Some(self.large_search_limit),
          Some(playlist_offset),
        )
        .await
      {
        self.set_playlist_tracks_to_table(&playlist_tracks).await;

        let mut app = self.app.lock().await;
        app.playlist_tracks = Some(playlist_tracks);
        app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
      };
    }
  }

  async fn set_playlist_tracks_to_table(&mut self, playlist_track_page: &Page<PlaylistItem>) {
    self
      .set_tracks_to_table(
        playlist_track_page
          .items
          .clone()
          .into_iter()
          .filter_map(|item| {
            item.track.and_then(|t| match t {
              PlayableItem::Track(track) => Some(track),
              _ => None,
            })
          })
          .collect::<Vec<FullTrack>>(),
      )
      .await;
  }

  async fn set_tracks_to_table(&mut self, tracks: Vec<FullTrack>) {
    let mut app = self.app.lock().await;
    app.track_table.tracks = tracks.clone();

    // Send this event round (don't block here)
    app.dispatch(IoEvent::CurrentUserSavedTracksContains(
      tracks
        .into_iter()
        .filter_map(|item| item.id.map(|id| id.id().to_string()))
        .collect::<Vec<String>>(),
    ));
  }

  async fn set_artists_to_table(&mut self, artists: Vec<FullArtist>) {
    let mut app = self.app.lock().await;
    app.artists = artists;
  }

  async fn get_made_for_you_playlist_tracks(
    &mut self,
    playlist_id: String,
    made_for_you_offset: u32,
  ) {
    if let Ok(playlist_id_obj) = PlaylistId::from_id(&playlist_id) {
      if let Ok(made_for_you_tracks) = self
        .spotify
        .playlist_items_manual(
          playlist_id_obj,
          None,
          None,
          Some(self.large_search_limit),
          Some(made_for_you_offset),
        )
        .await
      {
        self
          .set_playlist_tracks_to_table(&made_for_you_tracks)
          .await;

        let mut app = self.app.lock().await;
        app.made_for_you_tracks = Some(made_for_you_tracks);
        if app.get_current_route().id != RouteId::TrackTable {
          app.push_navigation_stack(RouteId::TrackTable, ActiveBlock::TrackTable);
        }
      }
    }
  }

  async fn get_current_user_saved_shows(&mut self, offset: Option<u32>) {
    match self
      .spotify
      .get_saved_show_manual(Some(self.large_search_limit), offset)
      .await
    {
      Ok(saved_shows) => {
        // not to show a blank page
        if !saved_shows.items.is_empty() {
          let mut app = self.app.lock().await;
          app.library.saved_shows.add_pages(saved_shows);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn current_user_saved_shows_contains(&mut self, show_ids: Vec<String>) {
    let show_id_refs: Vec<ShowId> = show_ids
      .iter()
      .filter_map(|id| ShowId::from_id(id).ok())
      .collect();
    
    if let Ok(are_followed) = self
      .spotify
      .check_users_saved_shows(show_id_refs.iter().map(|s| s.as_ref()))
      .await
    {
      let mut app = self.app.lock().await;
      show_ids.iter().enumerate().for_each(|(i, id)| {
        if are_followed[i] {
          app.saved_show_ids_set.insert(id.to_owned());
        } else {
          app.saved_show_ids_set.remove(id);
        }
      })
    }
  }

  async fn get_show_episodes(&mut self, show: Box<SimplifiedShow>) {
    match self
      .spotify
      .get_shows_episodes_manual(show.id.as_ref(), None::<Market>, Some(self.large_search_limit), Some(0))
      .await
    {
      Ok(episodes) => {
        if !episodes.items.is_empty() {
          let mut app = self.app.lock().await;
          app.library.show_episodes = ScrollableResultPages::new();
          app.library.show_episodes.add_pages(episodes);

          app.selected_show_simplified = Some(SelectedShow { show: *show });

          app.episode_table_context = EpisodeTableContext::Simplified;

          app.push_navigation_stack(RouteId::PodcastEpisodes, ActiveBlock::EpisodeTable);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_show(&mut self, show_id: String) {
    if let Ok(show_id_obj) = ShowId::from_id(&show_id) {
      match self.spotify.get_a_show(show_id_obj, None::<Market>).await {
        Ok(show) => {
          let selected_show = SelectedFullShow { show };

          let mut app = self.app.lock().await;

          app.selected_show_full = Some(selected_show);

          app.episode_table_context = EpisodeTableContext::Full;
          app.push_navigation_stack(RouteId::PodcastEpisodes, ActiveBlock::EpisodeTable);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn get_current_show_episodes(&mut self, show_id: String, offset: Option<u32>) {
    if let Ok(show_id_obj) = ShowId::from_id(&show_id) {
      match self
        .spotify
        .get_shows_episodes_manual(show_id_obj.as_ref(), None::<Market>, Some(self.large_search_limit), offset)
        .await
      {
        Ok(episodes) => {
          if !episodes.items.is_empty() {
            let mut app = self.app.lock().await;
            app.library.show_episodes.add_pages(episodes);
          }
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn get_search_results(&mut self, search_term: String, country: Option<Country>) {
    let market = country.map(Market::Country);
    
    let search_track = self.spotify.search(
      &search_term,
      SearchType::Track,
      market,
      None,
      Some(self.small_search_limit),
      Some(0),
    );

    let search_artist = self.spotify.search(
      &search_term,
      SearchType::Artist,
      market,
      None,
      Some(self.small_search_limit),
      Some(0),
    );

    let search_album = self.spotify.search(
      &search_term,
      SearchType::Album,
      market,
      None,
      Some(self.small_search_limit),
      Some(0),
    );

    let search_playlist = self.spotify.search(
      &search_term,
      SearchType::Playlist,
      market,
      None,
      Some(self.small_search_limit),
      Some(0),
    );

    let search_show = self.spotify.search(
      &search_term,
      SearchType::Show,
      market,
      None,
      Some(self.small_search_limit),
      Some(0),
    );

    // Run the futures concurrently
    match try_join!(
      search_track,
      search_artist,
      search_album,
      search_playlist,
      search_show
    ) {
      Ok((
        SearchResult::Tracks(track_results),
        SearchResult::Artists(artist_results),
        SearchResult::Albums(album_results),
        SearchResult::Playlists(playlist_results),
        SearchResult::Shows(show_results),
      )) => {
        let mut app = self.app.lock().await;

        let artist_ids: Vec<String> = album_results
          .items
          .iter()
          .filter_map(|item| item.id.as_ref().map(|id| id.id().to_string()))
          .collect();

        // Check if these artists are followed
        app.dispatch(IoEvent::UserArtistFollowCheck(artist_ids));

        let album_ids: Vec<String> = album_results
          .items
          .iter()
          .filter_map(|album| album.id.as_ref().map(|id| id.id().to_string()))
          .collect();

        // Check if these albums are saved
        app.dispatch(IoEvent::CurrentUserSavedAlbumsContains(album_ids));

        let show_ids: Vec<String> = show_results
          .items
          .iter()
          .map(|show| show.id.id().to_owned())
          .collect();

        // check if these shows are saved
        app.dispatch(IoEvent::CurrentUserSavedShowsContains(show_ids));

        app.search_results.tracks = Some(track_results);
        app.search_results.artists = Some(artist_results);
        app.search_results.albums = Some(album_results);
        app.search_results.playlists = Some(playlist_results);
        app.search_results.shows = Some(show_results);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
      _ => {}
    };
  }

  async fn get_current_user_saved_tracks(&mut self, offset: Option<u32>) {
    match self
      .spotify
      .current_user_saved_tracks_manual(None::<Market>, Some(self.large_search_limit), offset)
      .await
    {
      Ok(saved_tracks) => {
        let mut app = self.app.lock().await;
        app.track_table.tracks = saved_tracks
          .items
          .clone()
          .into_iter()
          .map(|item| item.track)
          .collect::<Vec<FullTrack>>();

        saved_tracks.items.iter().for_each(|item| {
          if let Some(ref track_id) = item.track.id {
            app.liked_song_ids_set.insert(track_id.id().to_string());
          }
        });

        app.library.saved_tracks.add_pages(saved_tracks);
        app.track_table.context = Some(TrackTableContext::SavedTracks);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn start_playback(
    &mut self,
    context_uri: Option<String>,
    uris: Option<Vec<String>>,
    offset: Option<usize>,
  ) {
    let playable_uris: Option<Vec<PlayableId>> = uris.as_ref().map(|u| {
      u.iter()
        .filter_map(|uri| {
          if uri.contains(":track:") {
            TrackId::from_uri(uri).ok().map(|t| PlayableId::Track(t))
          } else if uri.contains(":episode:") {
            EpisodeId::from_uri(uri).ok().map(|e| PlayableId::Episode(e))
          } else {
            None
          }
        })
        .collect()
    });
    
    let context_id: Option<PlayContextId> = context_uri.as_ref().and_then(|uri| {
      if uri.contains(":album:") {
        AlbumId::from_uri(uri).ok().map(PlayContextId::Album)
      } else if uri.contains(":playlist:") {
        PlaylistId::from_uri(uri).ok().map(PlayContextId::Playlist)
      } else if uri.contains(":artist:") {
        ArtistId::from_uri(uri).ok().map(PlayContextId::Artist)
      } else if uri.contains(":show:") {
        ShowId::from_uri(uri).ok().map(PlayContextId::Show)
      } else {
        None
      }
    });

    // For track position in a playlist/album, we use the Position offset
    // which in rspotify is actually a Duration (representing position index as milliseconds)
    let offset_obj = offset.map(|o| Offset::Position(chrono::Duration::try_milliseconds(o as i64).unwrap_or_default()));

    let device_id = self.client_config.device_id.as_deref();

    let result = match (&context_id, &playable_uris) {
      (Some(ctx), _) => {
        self.spotify.start_context_playback(ctx.as_ref(), device_id, offset_obj, None).await
      }
      (None, Some(uris)) if !uris.is_empty() => {
        let uris_refs: Vec<_> = uris.iter().map(|u| u.as_ref()).collect();
        self.spotify.start_uris_playback(uris_refs, device_id, offset_obj, None).await
      }
      _ => {
        self.spotify.resume_playback(device_id, None).await
      }
    };

    match result {
      Ok(()) => {
        let mut app = self.app.lock().await;
        app.song_progress_ms = 0;
        app.dispatch(IoEvent::GetCurrentPlayback);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn seek(&mut self, position_ms: u32) {
    let device_id = self.client_config.device_id.as_deref();
    match self
      .spotify
      .seek_track(chrono::Duration::try_milliseconds(position_ms as i64).unwrap(), device_id)
      .await
    {
      Ok(()) => {
        // Wait between seek and status query.
        tokio::time::sleep(Duration::from_millis(1000)).await;
        self.get_current_playback().await;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn next_track(&mut self) {
    match self
      .spotify
      .next_track(self.client_config.device_id.as_deref())
      .await
    {
      Ok(()) => {
        self.get_current_playback().await;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn previous_track(&mut self) {
    match self
      .spotify
      .previous_track(self.client_config.device_id.as_deref())
      .await
    {
      Ok(()) => {
        self.get_current_playback().await;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn shuffle(&mut self, shuffle_state: bool) {
    match self
      .spotify
      .shuffle(!shuffle_state, self.client_config.device_id.as_deref())
      .await
    {
      Ok(()) => {
        let mut app = self.app.lock().await;
        if let Some(current_playback_context) = &mut app.current_playback_context {
          current_playback_context.shuffle_state = !shuffle_state;
        };
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn repeat(&mut self, repeat_state: RepeatState) {
    let next_repeat_state = match repeat_state {
      RepeatState::Off => RepeatState::Context,
      RepeatState::Context => RepeatState::Track,
      RepeatState::Track => RepeatState::Off,
    };
    match self
      .spotify
      .repeat(next_repeat_state, self.client_config.device_id.as_deref())
      .await
    {
      Ok(()) => {
        let mut app = self.app.lock().await;
        if let Some(current_playback_context) = &mut app.current_playback_context {
          current_playback_context.repeat_state = next_repeat_state;
        };
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn pause_playback(&mut self) {
    match self
      .spotify
      .pause_playback(self.client_config.device_id.as_deref())
      .await
    {
      Ok(()) => {
        self.get_current_playback().await;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn change_volume(&mut self, volume_percent: u8) {
    match self
      .spotify
      .volume(volume_percent, self.client_config.device_id.as_deref())
      .await
    {
      Ok(()) => {
        let mut app = self.app.lock().await;
        if let Some(current_playback_context) = &mut app.current_playback_context {
          current_playback_context.device.volume_percent = Some(volume_percent.into());
        };
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn get_artist(
    &mut self,
    artist_id: String,
    input_artist_name: String,
    country: Option<Country>,
  ) {
    if let Ok(artist_id_obj) = ArtistId::from_id(&artist_id) {
      let market = country.map(Market::Country);
      
      let albums = self.spotify.artist_albums_manual(
        artist_id_obj.as_ref(),
        None,
        market,
        Some(self.large_search_limit),
        Some(0),
      );
      let artist_name = if input_artist_name.is_empty() {
        self
          .spotify
          .artist(artist_id_obj.as_ref())
          .await
          .map(|full_artist| full_artist.name)
          .unwrap_or_default()
      } else {
        input_artist_name
      };
      let top_tracks = self.spotify.artist_top_tracks(artist_id_obj.as_ref(), market);
      let related_artist = self.spotify.artist_related_artists(artist_id_obj.as_ref());

      if let Ok((albums, top_tracks, related_artist)) = try_join!(albums, top_tracks, related_artist)
      {
        let mut app = self.app.lock().await;

        app.dispatch(IoEvent::CurrentUserSavedAlbumsContains(
          albums
            .items
            .iter()
            .filter_map(|item| item.id.as_ref().map(|id| id.id().to_string()))
            .collect(),
        ));

        app.artist = Some(Artist {
          artist_name,
          albums,
          related_artists: related_artist,
          top_tracks: top_tracks.into_iter().collect(),
          selected_album_index: 0,
          selected_related_artist_index: 0,
          selected_top_track_index: 0,
          artist_hovered_block: ArtistBlock::TopTracks,
          artist_selected_block: ArtistBlock::Empty,
        });
      }
    }
  }

  async fn get_album_tracks(&mut self, album: Box<SimplifiedAlbum>) {
    if let Some(ref album_id) = album.id {
      match self
        .spotify
        .album_track_manual(album_id.as_ref(), None::<Market>, Some(self.large_search_limit), Some(0))
        .await
      {
        Ok(tracks) => {
          let track_ids = tracks
            .items
            .iter()
            .filter_map(|item| item.id.as_ref().map(|id| id.id().to_string()))
            .collect::<Vec<String>>();

          let mut app = self.app.lock().await;
          app.selected_album_simplified = Some(SelectedAlbum {
            album: *album,
            tracks,
            selected_index: 0,
          });

          app.album_table_context = AlbumTableContext::Simplified;
          app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
          app.dispatch(IoEvent::CurrentUserSavedTracksContains(track_ids));
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn get_recommendations_for_seed(
    &mut self,
    seed_artists: Option<Vec<String>>,
    seed_tracks: Option<Vec<String>>,
    first_track: Box<Option<FullTrack>>,
    country: Option<Country>,
  ) {
    let seed_artists_unwrapped = seed_artists.unwrap_or_default();
    let seed_artist_ids: Vec<ArtistId> = seed_artists_unwrapped
      .iter()
      .filter_map(|id| ArtistId::from_id(id).ok())
      .collect();
    
    let seed_tracks_unwrapped = seed_tracks.unwrap_or_default();
    let seed_track_ids: Vec<TrackId> = seed_tracks_unwrapped
      .iter()
      .filter_map(|id| TrackId::from_id(id).ok())
      .collect();

    let market = country.map(Market::Country);

    // seed_artists and seed_tracks need to be passed as owned iterators
    let seed_artists_opt: Option<Vec<ArtistId>> = if seed_artist_ids.is_empty() {
      None
    } else {
      Some(seed_artist_ids)
    };
    
    let seed_tracks_opt: Option<Vec<TrackId>> = if seed_track_ids.is_empty() {
      None
    } else {
      Some(seed_track_ids)
    };

    // Empty attributes
    let empty_attrs: Vec<rspotify::model::RecommendationsAttribute> = Vec::new();

    match self
      .spotify
      .recommendations(
        empty_attrs,
        seed_artists_opt,
        None::<Vec<&str>>,
        seed_tracks_opt,
        market,
        Some(self.large_search_limit),
      )
      .await
    {
      Ok(result) => {
        if let Some(mut recommended_tracks) = self.extract_recommended_tracks(&result).await {
          //custom first track
          if let Some(track) = *first_track {
            recommended_tracks.insert(0, track);
          }

          let track_ids = recommended_tracks
            .iter()
            .filter_map(|x| x.id.as_ref().map(|id| id.uri()))
            .collect::<Vec<String>>();

          self.set_tracks_to_table(recommended_tracks.clone()).await;

          let mut app = self.app.lock().await;
          app.recommended_tracks = recommended_tracks;
          app.track_table.context = Some(TrackTableContext::RecommendedTracks);

          if app.get_current_route().id != RouteId::Recommendations {
            app.push_navigation_stack(RouteId::Recommendations, ActiveBlock::TrackTable);
          };

          app.dispatch(IoEvent::StartPlayback(None, Some(track_ids), Some(0)));
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn extract_recommended_tracks(
    &mut self,
    recommendations: &Recommendations,
  ) -> Option<Vec<FullTrack>> {
    let track_ids: Vec<TrackId> = recommendations
      .tracks
      .iter()
      .filter_map(|item| item.id.clone())
      .collect();
    
    if track_ids.is_empty() {
      return None;
    }
    
    if let Ok(result) = self
      .spotify
      .tracks(track_ids.iter().map(|t| t.as_ref()), None::<Market>)
      .await
    {
      return Some(result);
    }

    None
  }

  async fn get_recommendations_for_track_id(&mut self, id: String, country: Option<Country>) {
    if let Ok(track_id) = TrackId::from_id(&id) {
      if let Ok(track) = self.spotify.track(track_id.as_ref(), None::<Market>).await {
        let track_id_list = track.id.as_ref().map(|id| vec![id.id().to_string()]);
        self
          .get_recommendations_for_seed(None, track_id_list, Box::new(Some(track)), country)
          .await;
      }
    }
  }

  async fn toggle_save_track(&mut self, track_id: String) {
    if let Ok(track_id_obj) = TrackId::from_id(&track_id) {
      match self
        .spotify
        .current_user_saved_tracks_contains([track_id_obj.as_ref()])
        .await
      {
        Ok(saved) => {
          if saved.first() == Some(&true) {
            match self
              .spotify
              .current_user_saved_tracks_delete([track_id_obj.as_ref()])
              .await
            {
              Ok(()) => {
                let mut app = self.app.lock().await;
                app.liked_song_ids_set.remove(&track_id);
              }
              Err(e) => {
                self.handle_error(anyhow!(e)).await;
              }
            }
          } else {
            match self
              .spotify
              .current_user_saved_tracks_add([track_id_obj.as_ref()])
              .await
            {
              Ok(()) => {
                let mut app = self.app.lock().await;
                app.liked_song_ids_set.insert(track_id);
              }
              Err(e) => {
                self.handle_error(anyhow!(e)).await;
              }
            }
          }
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      };
    }
  }

  async fn get_followed_artists(&mut self, after: Option<String>) {
    match self
      .spotify
      .current_user_followed_artists(after.as_deref(), Some(self.large_search_limit))
      .await
    {
      Ok(saved_artists) => {
        let mut app = self.app.lock().await;
        app.artists = saved_artists.items.to_owned();
        app.library.saved_artists.add_pages(saved_artists);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn user_artist_check_follow(&mut self, artist_ids: Vec<String>) {
    let artist_id_refs: Vec<ArtistId> = artist_ids
      .iter()
      .filter_map(|id| ArtistId::from_id(id).ok())
      .collect();
    
    if let Ok(are_followed) = self.spotify.user_artist_check_follow(artist_id_refs.iter().map(|a| a.as_ref())).await {
      let mut app = self.app.lock().await;
      artist_ids.iter().enumerate().for_each(|(i, id)| {
        if are_followed[i] {
          app.followed_artist_ids_set.insert(id.to_owned());
        } else {
          app.followed_artist_ids_set.remove(id);
        }
      });
    }
  }

  async fn get_current_user_saved_albums(&mut self, offset: Option<u32>) {
    match self
      .spotify
      .current_user_saved_albums_manual(None::<Market>, Some(self.large_search_limit), offset)
      .await
    {
      Ok(saved_albums) => {
        // not to show a blank page
        if !saved_albums.items.is_empty() {
          let mut app = self.app.lock().await;
          app.library.saved_albums.add_pages(saved_albums);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn current_user_saved_albums_contains(&mut self, album_ids: Vec<String>) {
    let album_id_refs: Vec<AlbumId> = album_ids
      .iter()
      .filter_map(|id| AlbumId::from_id(id).ok())
      .collect();
    
    if let Ok(are_followed) = self
      .spotify
      .current_user_saved_albums_contains(album_id_refs.iter().map(|a| a.as_ref()))
      .await
    {
      let mut app = self.app.lock().await;
      album_ids.iter().enumerate().for_each(|(i, id)| {
        if are_followed[i] {
          app.saved_album_ids_set.insert(id.to_owned());
        } else {
          app.saved_album_ids_set.remove(id);
        }
      });
    }
  }

  pub async fn current_user_saved_album_delete(&mut self, album_id: String) {
    if let Ok(album_id_obj) = AlbumId::from_id(&album_id) {
      match self
        .spotify
        .current_user_saved_albums_delete([album_id_obj.as_ref()])
        .await
      {
        Ok(_) => {
          self.get_current_user_saved_albums(None).await;
          let mut app = self.app.lock().await;
          app.saved_album_ids_set.remove(&album_id);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      };
    }
  }

  async fn current_user_saved_album_add(&mut self, album_id: String) {
    if let Ok(album_id_obj) = AlbumId::from_id(&album_id) {
      match self
        .spotify
        .current_user_saved_albums_add([album_id_obj.as_ref()])
        .await
      {
        Ok(_) => {
          let mut app = self.app.lock().await;
          app.saved_album_ids_set.insert(album_id);
        }
        Err(e) => self.handle_error(anyhow!(e)).await,
      }
    }
  }

  async fn current_user_saved_shows_delete(&mut self, show_id: String) {
    if let Ok(show_id_obj) = ShowId::from_id(&show_id) {
      match self
        .spotify
        .remove_users_saved_shows([show_id_obj.as_ref()], None::<Market>)
        .await
      {
        Ok(_) => {
          self.get_current_user_saved_shows(None).await;
          let mut app = self.app.lock().await;
          app.saved_show_ids_set.remove(&show_id);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn current_user_saved_shows_add(&mut self, show_id: String) {
    if let Ok(show_id_obj) = ShowId::from_id(&show_id) {
      match self.spotify.save_shows([show_id_obj.as_ref()]).await {
        Ok(_) => {
          self.get_current_user_saved_shows(None).await;
          let mut app = self.app.lock().await;
          app.saved_show_ids_set.insert(show_id);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn user_unfollow_artists(&mut self, artist_ids: Vec<String>) {
    let artist_id_refs: Vec<ArtistId> = artist_ids
      .iter()
      .filter_map(|id| ArtistId::from_id(id).ok())
      .collect();
    
    match self.spotify.user_unfollow_artists(artist_id_refs.iter().map(|a| a.as_ref())).await {
      Ok(_) => {
        self.get_followed_artists(None).await;
        let mut app = self.app.lock().await;
        artist_ids.iter().for_each(|id| {
          app.followed_artist_ids_set.remove(id);
        });
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn user_follow_artists(&mut self, artist_ids: Vec<String>) {
    let artist_id_refs: Vec<ArtistId> = artist_ids
      .iter()
      .filter_map(|id| ArtistId::from_id(id).ok())
      .collect();
    
    match self.spotify.user_follow_artists(artist_id_refs.iter().map(|a| a.as_ref())).await {
      Ok(_) => {
        self.get_followed_artists(None).await;
        let mut app = self.app.lock().await;
        artist_ids.iter().for_each(|id| {
          app.followed_artist_ids_set.insert(id.to_owned());
        });
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn user_follow_playlist(
    &mut self,
    _playlist_owner_id: String,
    playlist_id: String,
    is_public: Option<bool>,
  ) {
    if let Ok(playlist_id_obj) = PlaylistId::from_id(&playlist_id) {
      match self
        .spotify
        .playlist_follow(playlist_id_obj, is_public)
        .await
      {
        Ok(_) => {
          self.get_current_user_playlists().await;
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn user_unfollow_playlist(&mut self, _user_id: String, playlist_id: String) {
    if let Ok(playlist_id_obj) = PlaylistId::from_id(&playlist_id) {
      match self
        .spotify
        .playlist_unfollow(playlist_id_obj)
        .await
      {
        Ok(_) => {
          self.get_current_user_playlists().await;
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn made_for_you_search_and_add(&mut self, search_string: String, country: Option<Country>) {
    const SPOTIFY_ID: &str = "spotify";
    let market = country.map(Market::Country);

    match self
      .spotify
      .search(
        &search_string,
        SearchType::Playlist,
        market,
        None,
        Some(self.large_search_limit),
        Some(0),
      )
      .await
    {
      Ok(SearchResult::Playlists(mut search_playlists)) => {
        let mut filtered_playlists = search_playlists
          .items
          .iter()
          .filter(|playlist| playlist.owner.id.id() == SPOTIFY_ID && playlist.name == search_string)
          .map(|playlist| playlist.to_owned())
          .collect::<Vec<SimplifiedPlaylist>>();

        let mut app = self.app.lock().await;
        if !app.library.made_for_you_playlists.pages.is_empty() {
          app
            .library
            .made_for_you_playlists
            .get_mut_results(None)
            .unwrap()
            .items
            .append(&mut filtered_playlists);
        } else {
          search_playlists.items = filtered_playlists;
          app
            .library
            .made_for_you_playlists
            .add_pages(search_playlists);
        }
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
      _ => {}
    }
  }

  async fn get_audio_analysis(&mut self, uri: String) {
    // Extract track ID from URI
    if let Some(track_id_str) = uri.strip_prefix("spotify:track:") {
      if let Ok(track_id) = TrackId::from_id(track_id_str) {
        match self.spotify.track_analysis(track_id.as_ref()).await {
          Ok(result) => {
            let mut app = self.app.lock().await;
            app.audio_analysis = Some(result);
          }
          Err(e) => {
            self.handle_error(anyhow!(e)).await;
          }
        }
      }
    }
  }

  async fn get_current_user_playlists(&mut self) {
    let playlists = self
      .spotify
      .current_user_playlists_manual(Some(self.large_search_limit), None)
      .await;

    match playlists {
      Ok(p) => {
        let mut app = self.app.lock().await;
        app.playlists = Some(p);
        // Select the first playlist
        app.selected_playlist_index = Some(0);
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    };
  }

  async fn get_recently_played(&mut self) {
    match self
      .spotify
      .current_user_recently_played(Some(self.large_search_limit), None)
      .await
    {
      Ok(result) => {
        let track_ids = result
          .items
          .iter()
          .filter_map(|item| item.track.id.as_ref().map(|id| id.id().to_string()))
          .collect::<Vec<String>>();

        self.current_user_saved_tracks_contains(track_ids).await;

        let mut app = self.app.lock().await;

        app.recently_played.result = Some(result.clone());
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
      }
    }
  }

  async fn get_album(&mut self, album_id: String) {
    if let Ok(album_id_obj) = AlbumId::from_id(&album_id) {
      match self.spotify.album(album_id_obj.as_ref(), None::<Market>).await {
        Ok(album) => {
          let selected_album = SelectedFullAlbum {
            album,
            selected_index: 0,
          };

          let mut app = self.app.lock().await;

          app.selected_album_full = Some(selected_album);
          app.album_table_context = AlbumTableContext::Full;
          app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn get_album_for_track(&mut self, track_id: String) {
    if let Ok(track_id_obj) = TrackId::from_id(&track_id) {
      match self.spotify.track(track_id_obj.as_ref(), None::<Market>).await {
        Ok(track) => {
          let album_id = match track.album.id {
            Some(ref id) => id.clone(),
            None => return,
          };

          if let Ok(album) = self.spotify.album(album_id.as_ref(), None::<Market>).await {
            let zero_indexed_track_number = track.track_number - 1;
            let selected_album = SelectedFullAlbum {
              album,
              selected_index: zero_indexed_track_number as usize,
            };

            let mut app = self.app.lock().await;

            app.selected_album_full = Some(selected_album.clone());
            app.saved_album_tracks_index = selected_album.selected_index;
            app.album_table_context = AlbumTableContext::Full;
            app.push_navigation_stack(RouteId::AlbumTracks, ActiveBlock::AlbumTracks);
          }
        }
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }

  async fn transfer_playback_to_device(&mut self, device_id: String) {
    match self.spotify.transfer_playback(&device_id, Some(true)).await {
      Ok(()) => {
        self.get_current_playback().await;
      }
      Err(e) => {
        self.handle_error(anyhow!(e)).await;
        return;
      }
    };

    match self.client_config.set_device_id(device_id) {
      Ok(()) => {
        let mut app = self.app.lock().await;
        app.pop_navigation_stack();
      }
      Err(e) => {
        self.handle_error(e).await;
      }
    };
  }

  async fn refresh_authentication(&mut self) {
    // Try to refresh the token
    match self.spotify.refresh_token().await {
      Ok(()) => {
        if let Ok(token_lock) = self.spotify.token.lock().await {
          if let Some(ref token) = *token_lock {
            let mut app = self.app.lock().await;
            app.spotify_token_expiry = token.expires_at.unwrap_or(Utc::now() + TimeDelta::try_hours(1).unwrap());
            
            // Save token to cache
            let token_path = crate::config::get_token_cache_path();
            let _ = token.write_cache(&token_path);
          }
        }
      }
      Err(e) => {
        println!("\nFailed to refresh authentication token: {:?}", e);
      }
    }
  }

  async fn add_item_to_queue(&mut self, item: String) {
    // Parse the item as a playable ID
    let playable_id: Option<PlayableId> = if item.contains(":track:") {
      TrackId::from_uri(&item).ok().map(PlayableId::Track)
    } else if item.contains(":episode:") {
      EpisodeId::from_uri(&item).ok().map(PlayableId::Episode)
    } else {
      None
    };
    
    if let Some(id) = playable_id {
      match self
        .spotify
        .add_item_to_queue(id.as_ref(), self.client_config.device_id.as_deref())
        .await
      {
        Ok(()) => (),
        Err(e) => {
          self.handle_error(anyhow!(e)).await;
        }
      }
    }
  }
}
