package com.bluewave.player

import android.content.ContentUris
import android.content.Context
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.provider.MediaStore
import android.provider.OpenableColumns
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID

data class Track(
    val id: String,
    val title: String,
    val artist: String,
    val album: String,
    val durationMs: Long,
    val path: String,
    val artPath: String?,
    val source: String,
    val addedAt: Long,
    val favorite: Boolean = false,
    val playCount: Int = 0,
    val lastPlayed: Long = 0,
) {
    val uri: Uri get() = Uri.fromFile(File(path))
    val artUri: Uri? get() = artPath?.let { Uri.fromFile(File(it)) }
}

data class Playlist(val id: String, val name: String, val trackIds: List<String>, val createdAt: Long)

data class ScanProgress(val running: Boolean = false, val done: Int = 0, val total: Int = 0)

/**
 * Library is stored entirely in the app's private storage (filesDir).
 * Audio files are COPIED there, so they keep working after the originals are deleted
 * and are invisible to the phone's file manager / other apps.
 */
object Library {
    private lateinit var ctx: Context
    private val mutex = Mutex()
    private val _tracks = MutableStateFlow<List<Track>>(emptyList())
    val tracks: StateFlow<List<Track>> = _tracks.asStateFlow()
    private val _playlists = MutableStateFlow<List<Playlist>>(emptyList())
    val playlists: StateFlow<List<Playlist>> = _playlists.asStateFlow()
    private val _scan = MutableStateFlow(ScanProgress())
    val scan: StateFlow<ScanProgress> = _scan.asStateFlow()

    private val ignoredSources = mutableSetOf<String>()
    private var loaded = false

    private val musicDir get() = File(ctx.filesDir, "music").apply { mkdirs() }
    private val artDir get() = File(ctx.filesDir, "art").apply { mkdirs() }
    private val dbFile get() = File(ctx.filesDir, "library.json")

    fun init(context: Context) {
        if (loaded) return
        ctx = context.applicationContext
        load()
        loaded = true
    }

    fun byId(id: String?): Track? = _tracks.value.firstOrNull { it.id == id }

    // ---------- persistence ----------
    private fun load() {
        val f = dbFile
        if (!f.exists()) return
        runCatching {
            val o = JSONObject(f.readText())
            val ts = o.optJSONArray("tracks") ?: JSONArray()
            val list = ArrayList<Track>()
            for (i in 0 until ts.length()) {
                val t = ts.getJSONObject(i)
                val path = t.getString("path")
                if (!File(path).exists()) continue
                list += Track(
                    id = t.getString("id"),
                    title = t.optString("title"),
                    artist = t.optString("artist"),
                    album = t.optString("album"),
                    durationMs = t.optLong("duration"),
                    path = path,
                    artPath = t.optString("art").ifEmpty { null },
                    source = t.optString("source"),
                    addedAt = t.optLong("addedAt"),
                    favorite = t.optBoolean("fav"),
                    playCount = t.optInt("plays"),
                    lastPlayed = t.optLong("lastPlayed"),
                )
            }
            _tracks.value = list
            val ps = o.optJSONArray("playlists") ?: JSONArray()
            val pls = ArrayList<Playlist>()
            for (i in 0 until ps.length()) {
                val p = ps.getJSONObject(i)
                val ids = p.optJSONArray("tracks") ?: JSONArray()
                pls += Playlist(p.getString("id"), p.getString("name"),
                    List(ids.length()) { ids.getString(it) }, p.optLong("createdAt"))
            }
            _playlists.value = pls
            val ig = o.optJSONArray("ignored") ?: JSONArray()
            for (i in 0 until ig.length()) ignoredSources += ig.getString(i)
        }
    }

    private fun save() {
        val o = JSONObject()
        o.put("tracks", JSONArray().apply {
            _tracks.value.forEach { t ->
                put(JSONObject().apply {
                    put("id", t.id); put("title", t.title); put("artist", t.artist)
                    put("album", t.album); put("duration", t.durationMs); put("path", t.path)
                    put("art", t.artPath ?: ""); put("source", t.source); put("addedAt", t.addedAt)
                    put("fav", t.favorite); put("plays", t.playCount); put("lastPlayed", t.lastPlayed)
                })
            }
        })
        o.put("playlists", JSONArray().apply {
            _playlists.value.forEach { p ->
                put(JSONObject().apply {
                    put("id", p.id); put("name", p.name); put("createdAt", p.createdAt)
                    put("tracks", JSONArray(p.trackIds))
                })
            }
        })
        o.put("ignored", JSONArray(ignoredSources.toList()))
        val tmp = File(ctx.filesDir, "library.json.tmp")
        tmp.writeText(o.toString())
        tmp.renameTo(dbFile)
    }

    private suspend fun mutate(block: () -> Unit) = withContext(Dispatchers.IO) {
        mutex.withLock { block(); save() }
    }

    // ---------- scanning / import ----------
    /** Scans the device MediaStore and copies every new audio file into private storage. */
    suspend fun scanDevice(): Int = withContext(Dispatchers.IO) {
        if (_scan.value.running) return@withContext 0
        val found = ArrayList<Pair<String, Uri>>() // source key, uri
        val names = HashMap<String, String>()
        val collection = MediaStore.Audio.Media.EXTERNAL_CONTENT_URI
        val proj = arrayOf(
            MediaStore.Audio.Media._ID, MediaStore.Audio.Media.DISPLAY_NAME,
            MediaStore.Audio.Media.SIZE, MediaStore.Audio.Media.DURATION,
        )
        runCatching {
            ctx.contentResolver.query(
                collection, proj,
                "${MediaStore.Audio.Media.IS_MUSIC} != 0 OR ${MediaStore.Audio.Media.DURATION} >= 30000",
                null, null
            )?.use { c ->
                val iId = c.getColumnIndexOrThrow(MediaStore.Audio.Media._ID)
                val iName = c.getColumnIndexOrThrow(MediaStore.Audio.Media.DISPLAY_NAME)
                val iSize = c.getColumnIndexOrThrow(MediaStore.Audio.Media.SIZE)
                val iDur = c.getColumnIndexOrThrow(MediaStore.Audio.Media.DURATION)
                while (c.moveToNext()) {
                    val dur = c.getLong(iDur)
                    if (dur in 1..9_999) continue // skip short sounds / notifications
                    val name = c.getString(iName) ?: "track"
                    val key = "dev:$name:${c.getLong(iSize)}"
                    val uri = ContentUris.withAppendedId(collection, c.getLong(iId))
                    found += key to uri
                    names[key] = name
                }
            }
        }
        val existing = _tracks.value.map { it.source }.toHashSet()
        val todo = found.filter { it.first !in existing && it.first !in ignoredSources }
            .distinctBy { it.first }
        if (todo.isEmpty()) return@withContext 0
        _scan.value = ScanProgress(true, 0, todo.size)
        var added = 0
        todo.forEachIndexed { i, (key, uri) ->
            importOne(uri, key, names[key])?.let { added++ }
            _scan.value = ScanProgress(true, i + 1, todo.size)
        }
        _scan.value = ScanProgress()
        added
    }

    /** Import files chosen by the user from the system picker. */
    suspend fun importUris(uris: List<Uri>): Int = withContext(Dispatchers.IO) {
        _scan.value = ScanProgress(true, 0, uris.size)
        var added = 0
        uris.forEachIndexed { i, uri ->
            var name: String? = null
            var size = 0L
            runCatching {
                ctx.contentResolver.query(uri, null, null, null, null)?.use { c ->
                    if (c.moveToFirst()) {
                        val n = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                        val s = c.getColumnIndex(OpenableColumns.SIZE)
                        if (n >= 0) name = c.getString(n)
                        if (s >= 0) size = c.getLong(s)
                    }
                }
            }
            val key = "dev:${name ?: uri}:$size"
            ignoredSources.remove(key)
            if (_tracks.value.none { it.source == key }) {
                importOne(uri, key, name)?.let { added++ }
            }
            _scan.value = ScanProgress(true, i + 1, uris.size)
        }
        _scan.value = ScanProgress()
        added
    }

    private suspend fun importOne(uri: Uri, source: String, displayName: String?): Track? {
        val id = UUID.randomUUID().toString()
        val ext = displayName?.substringAfterLast('.', "")?.takeIf { it.length in 1..5 } ?: "audio"
        val dest = File(musicDir, "$id.$ext")
        try {
            ctx.contentResolver.openInputStream(uri)?.use { input ->
                dest.outputStream().use { input.copyTo(it, 256 * 1024) }
            } ?: return null
        } catch (e: Exception) {
            dest.delete(); return null
        }
        val mmr = MediaMetadataRetriever()
        var title: String? = null
        var artist: String? = null
        var album: String? = null
        var dur = 0L
        var art: String? = null
        try {
            mmr.setDataSource(dest.absolutePath)
            title = mmr.extractMetadata(MediaMetadataRetriever.METADATA_KEY_TITLE)
            artist = mmr.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ARTIST)
                ?: mmr.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ALBUMARTIST)
            album = mmr.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ALBUM)
            dur = mmr.extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0
            mmr.embeddedPicture?.let { bytes ->
                val f = File(artDir, "$id.jpg"); f.writeBytes(bytes); art = f.absolutePath
            }
        } catch (_: Exception) {
            dest.delete(); return null // not a playable audio file
        } finally {
            runCatching { mmr.release() }
        }
        val fallback = displayName?.substringBeforeLast('.') ?: "Без названия"
        var t = title?.takeIf { it.isNotBlank() } ?: fallback
        var a = artist?.takeIf { it.isNotBlank() && it != "<unknown>" }
        if (a == null && title.isNullOrBlank() && fallback.contains(" - ")) {
            a = fallback.substringBefore(" - ").trim(); t = fallback.substringAfter(" - ").trim()
        }
        val track = Track(
            id = id, title = t, artist = a ?: "Неизвестный исполнитель",
            album = album?.takeIf { it.isNotBlank() } ?: "Без альбома",
            durationMs = dur, path = dest.absolutePath, artPath = art,
            source = source, addedAt = System.currentTimeMillis(),
        )
        mutate { _tracks.value = _tracks.value + track }
        return track
    }

    // ---------- edits ----------
    suspend fun toggleFavorite(id: String) = mutate {
        _tracks.value = _tracks.value.map { if (it.id == id) it.copy(favorite = !it.favorite) else it }
    }

    suspend fun markPlayed(id: String) = mutate {
        _tracks.value = _tracks.value.map {
            if (it.id == id) it.copy(playCount = it.playCount + 1, lastPlayed = System.currentTimeMillis()) else it
        }
    }

    suspend fun editTrack(id: String, title: String, artist: String, album: String) = mutate {
        _tracks.value = _tracks.value.map {
            if (it.id == id) it.copy(title = title, artist = artist, album = album) else it
        }
    }

    suspend fun deleteTrack(id: String) = mutate {
        val t = _tracks.value.firstOrNull { it.id == id } ?: return@mutate
        File(t.path).delete(); t.artPath?.let { File(it).delete() }
        ignoredSources += t.source // don't re-import on next scan
        _tracks.value = _tracks.value.filter { it.id != id }
        _playlists.value = _playlists.value.map { p -> p.copy(trackIds = p.trackIds.filter { it != id }) }
    }

    suspend fun createPlaylist(name: String): Playlist {
        val p = Playlist(UUID.randomUUID().toString(), name, emptyList(), System.currentTimeMillis())
        mutate { _playlists.value = _playlists.value + p }
        return p
    }

    suspend fun renamePlaylist(id: String, name: String) = mutate {
        _playlists.value = _playlists.value.map { if (it.id == id) it.copy(name = name) else it }
    }

    suspend fun deletePlaylist(id: String) = mutate {
        _playlists.value = _playlists.value.filter { it.id != id }
    }

    suspend fun addToPlaylist(pid: String, trackId: String) = mutate {
        _playlists.value = _playlists.value.map {
            if (it.id == pid && trackId !in it.trackIds) it.copy(trackIds = it.trackIds + trackId) else it
        }
    }

    suspend fun removeFromPlaylist(pid: String, trackId: String) = mutate {
        _playlists.value = _playlists.value.map {
            if (it.id == pid) it.copy(trackIds = it.trackIds - trackId) else it
        }
    }

    fun storageBytes(): Long = musicDir.walkTopDown().filter { it.isFile }.sumOf { it.length() }
}
