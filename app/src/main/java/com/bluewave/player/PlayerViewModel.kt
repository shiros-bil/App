package com.bluewave.player

import android.app.Application
import android.content.ComponentName
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.guava.await
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

data class PlayerUi(
    val currentId: String? = null,
    val isPlaying: Boolean = false,
    val position: Long = 0,
    val duration: Long = 0,
    val shuffle: Boolean = false,
    val repeat: Int = Player.REPEAT_MODE_OFF,
    val queue: List<String> = emptyList(),
    val index: Int = 0,
    val speed: Float = 1f,
    val sleepAt: Long = 0,
)

class PlayerViewModel(app: Application) : AndroidViewModel(app) {
    private var controller: MediaController? = null
    private val _ui = MutableStateFlow(PlayerUi())
    val ui = _ui.asStateFlow()
    private var sleepJob: Job? = null

    init {
        Library.init(app)
        viewModelScope.launch {
            val token = SessionToken(app, ComponentName(app, PlaybackService::class.java))
            val c = MediaController.Builder(app, token).buildAsync().await()
            controller = c
            c.addListener(object : Player.Listener {
                override fun onEvents(player: Player, events: Player.Events) = refresh()
            })
            refresh()
            while (isActive) { // progress ticker
                if (c.isPlaying) refresh()
                delay(500)
            }
        }
    }

    private fun refresh() {
        val c = controller ?: return
        val q = List(c.mediaItemCount) { c.getMediaItemAt(it).mediaId }
        _ui.value = _ui.value.copy(
            currentId = c.currentMediaItem?.mediaId,
            isPlaying = c.isPlaying,
            position = c.currentPosition.coerceAtLeast(0),
            duration = c.duration.takeIf { it > 0 } ?: (Library.byId(c.currentMediaItem?.mediaId)?.durationMs ?: 0),
            shuffle = c.shuffleModeEnabled,
            repeat = c.repeatMode,
            queue = q,
            index = c.currentMediaItemIndex,
            speed = c.playbackParameters.speed,
        )
    }

    private fun Track.toItem(): MediaItem = MediaItem.Builder()
        .setMediaId(id)
        .setUri(uri)
        .setMediaMetadata(
            MediaMetadata.Builder()
                .setTitle(title).setArtist(artist).setAlbumTitle(album)
                .setArtworkUri(artUri)
                .setIsPlayable(true).setIsBrowsable(false)
                .build()
        ).build()

    fun play(list: List<Track>, start: Int = 0, shuffle: Boolean = false) {
        val c = controller ?: return
        if (list.isEmpty()) return
        c.shuffleModeEnabled = shuffle
        val startIdx = if (shuffle) list.indices.random() else start
        c.setMediaItems(list.map { it.toItem() }, startIdx, 0)
        c.prepare(); c.play()
    }

    fun playNext(t: Track) {
        val c = controller ?: return
        if (c.mediaItemCount == 0) return play(listOf(t))
        c.addMediaItem(c.currentMediaItemIndex + 1, t.toItem())
    }

    fun addToQueue(t: Track) {
        val c = controller ?: return
        if (c.mediaItemCount == 0) return play(listOf(t))
        c.addMediaItem(t.toItem())
    }

    fun removeFromQueue(i: Int) { controller?.removeMediaItem(i) }
    fun moveInQueue(from: Int, to: Int) { controller?.moveMediaItem(from, to) }
    fun jumpTo(i: Int) { controller?.run { seekTo(i, 0); play() } }
    fun toggle() { controller?.run { if (isPlaying) pause() else { if (playbackState == Player.STATE_ENDED) seekTo(0); play() } } }
    fun next() { controller?.seekToNext() }
    fun prev() { controller?.seekToPrevious() }
    fun seek(ms: Long) { controller?.seekTo(ms); refresh() }
    fun toggleShuffle() { controller?.run { shuffleModeEnabled = !shuffleModeEnabled } }
    fun cycleRepeat() {
        controller?.run {
            repeatMode = when (repeatMode) {
                Player.REPEAT_MODE_OFF -> Player.REPEAT_MODE_ALL
                Player.REPEAT_MODE_ALL -> Player.REPEAT_MODE_ONE
                else -> Player.REPEAT_MODE_OFF
            }
        }
    }
    fun setSpeed(s: Float) { controller?.setPlaybackSpeed(s) }

    fun onTrackDeleted(id: String) {
        val c = controller ?: return
        for (i in c.mediaItemCount - 1 downTo 0) if (c.getMediaItemAt(i).mediaId == id) c.removeMediaItem(i)
    }

    fun sleepTimer(minutes: Int) {
        sleepJob?.cancel()
        if (minutes <= 0) { _ui.value = _ui.value.copy(sleepAt = 0); return }
        val at = System.currentTimeMillis() + minutes * 60_000L
        _ui.value = _ui.value.copy(sleepAt = at)
        sleepJob = viewModelScope.launch {
            delay(minutes * 60_000L)
            controller?.pause()
            _ui.value = _ui.value.copy(sleepAt = 0)
        }
    }

    override fun onCleared() {
        controller?.release()
        super.onCleared()
    }
}

