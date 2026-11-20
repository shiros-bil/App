package com.bluewave.player

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.PlaylistAdd
import androidx.compose.material.icons.automirrored.filled.PlaylistPlay
import androidx.compose.material.icons.automirrored.filled.QueueMusic
import androidx.compose.material.icons.filled.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.media3.common.Player
import coil.compose.AsyncImage
import kotlinx.coroutines.launch

// ---------------- palette: black / white / blue ----------------
val Bg = Color(0xFF000000)
val Surface1 = Color(0xFF0E0F14)
val Surface2 = Color(0xFF181A22)
val Blue = Color(0xFF3D8BFF)
val BlueDeep = Color(0xFF1B3B78)
val TextHi = Color(0xFFFFFFFF)
val TextLo = Color(0xFF9097A6)

private val scheme = darkColorScheme(
    primary = Blue, onPrimary = Color.White, secondary = Blue,
    background = Bg, onBackground = TextHi, surface = Surface1, onSurface = TextHi,
    surfaceVariant = Surface2, onSurfaceVariant = TextLo, surfaceContainer = Surface1,
    surfaceContainerHigh = Surface2, surfaceContainerHighest = Surface2,
    surfaceContainerLow = Surface1, outline = Color(0xFF2A2D38),
)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        Library.init(this)
        setContent { MaterialTheme(colorScheme = scheme) { App() } }
    }
}

fun audioPermission() =
    if (Build.VERSION.SDK_INT >= 33) Manifest.permission.READ_MEDIA_AUDIO else Manifest.permission.READ_EXTERNAL_STORAGE

fun hasAudioPerm(ctx: Context) =
    ContextCompat.checkSelfPermission(ctx, audioPermission()) == PackageManager.PERMISSION_GRANTED

fun fmt(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return if (s >= 3600) "%d:%02d:%02d".format(s / 3600, s / 60 % 60, s % 60) else "%d:%02d".format(s / 60, s % 60)
}

sealed class Screen {
    data object Home : Screen()
    data object Library : Screen()
    data object Search : Screen()
    data object Settings : Screen()
    data class Album(val name: String) : Screen()
    data class Artist(val name: String) : Screen()
    data class PlaylistS(val id: String) : Screen()
    data object Favorites : Screen()
    data object Recent : Screen()
}

// ---------------- root ----------------
@Composable
fun App() {
    val ctx = LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("app", Context.MODE_PRIVATE) }
    var onboarded by remember { mutableStateOf(prefs.getBoolean("onboarded", false)) }
    val scope = rememberCoroutineScope()

    if (!onboarded) {
        Onboarding {
            prefs.edit().putBoolean("onboarded", true).apply()
            onboarded = true
        }
        return
    }

    // Auto-scan for newly downloaded tracks every time the app comes to foreground
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        if (hasAudioPerm(ctx) && prefs.getBoolean("autoscan", true)) scope.launch { Library.scanDevice() }
    }
    MainScaffold()
}

@Composable
fun Onboarding(onDone: () -> Unit) {
    val ctx = LocalContext.current
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { onDone() }
    Box(
        Modifier.fillMaxSize()
            .background(Brush.verticalGradient(listOf(BlueDeep, Bg, Bg)))
            .systemBarsPadding().padding(28.dp)
    ) {
        Column(Modifier.align(Alignment.Center), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(
                Modifier.size(120.dp).clip(RoundedCornerShape(32.dp)).background(Surface2),
                contentAlignment = Alignment.Center
            ) { Icon(Icons.Default.GraphicEq, null, tint = Blue, modifier = Modifier.size(64.dp)) }
            Spacer(Modifier.height(28.dp))
            Text("BlueWave", fontSize = 36.sp, fontWeight = FontWeight.Black, color = TextHi)
            Spacer(Modifier.height(12.dp))
            Text(
                "Вся твоя музыка в одном месте. Разреши доступ к аудиофайлам — приложение само найдёт и загрузит все скачанные треки в свою защищённую медиатеку. Треки будут работать, даже если удалить их из памяти телефона.",
                color = TextLo, fontSize = 15.sp, lineHeight = 22.sp,
                textAlign = androidx.compose.ui.text.style.TextAlign.Center
            )
            Spacer(Modifier.height(36.dp))
            Button(
                onClick = {
                    val perms = mutableListOf(audioPermission())
                    if (Build.VERSION.SDK_INT >= 33) perms += Manifest.permission.POST_NOTIFICATIONS
                    if (perms.all { ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED }) onDone()
                    else launcher.launch(perms.toTypedArray())
                },
                modifier = Modifier.fillMaxWidth().height(56.dp),
                shape = RoundedCornerShape(28.dp),
                colors = ButtonDefaults.buttonColors(containerColor = Blue)
            ) { Text("Разрешить доступ к музыке", fontSize = 16.sp, fontWeight = FontWeight.Bold) }
            TextButton(onClick = onDone) { Text("Позже", color = TextLo) }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScaffold(vm: PlayerViewModel = viewModel()) {
    val tracks by Library.tracks.collectAsStateWithLifecycle()
    val ui by vm.ui.collectAsStateWithLifecycle()
    val stack = remember { mutableStateListOf<Screen>(Screen.Home) }
    var tab by remember { mutableStateOf<Screen>(Screen.Home) }
    var playerOpen by remember { mutableStateOf(false) }
    var menuTrack by remember { mutableStateOf<Track?>(null) }
    val current = stack.last()
    val nav: (Screen) -> Unit = { stack.add(it) }
    val current_track = tracks.firstOrNull { it.id == ui.currentId }

    BackHandler(enabled = playerOpen || stack.size > 1) {
        if (playerOpen) playerOpen = false else stack.removeAt(stack.lastIndex)
    }

    Box(Modifier.fillMaxSize().background(Bg)) {
        Scaffold(
            containerColor = Bg,
            bottomBar = {
                Column(Modifier.background(Bg)) {
                    if (current_track != null) MiniPlayer(current_track, ui, vm) { playerOpen = true }
                    NavigationBar(containerColor = Bg, tonalElevation = 0.dp) {
                        listOf(
                            Triple(Screen.Home, Icons.Default.Home, "Главная"),
                            Triple(Screen.Search, Icons.Default.Search, "Поиск"),
                            Triple(Screen.Library, Icons.Default.LibraryMusic, "Медиатека"),
                            Triple(Screen.Settings, Icons.Default.Settings, "Настройки"),
                        ).forEach { (s, ic, label) ->
                            NavigationBarItem(
                                selected = tab == s,
                                onClick = { tab = s; stack.clear(); stack.add(s) },
                                icon = { Icon(ic, null) }, label = { Text(label) },
                                colors = NavigationBarItemDefaults.colors(
                                    selectedIconColor = Blue, selectedTextColor = Blue,
                                    indicatorColor = Surface2, unselectedIconColor = TextLo, unselectedTextColor = TextLo
                                )
                            )
                        }
                    }
                }
            }
        ) { pad ->
            Box(Modifier.padding(pad).fillMaxSize()) {
                val onMenu: (Track) -> Unit = { menuTrack = it }
                when (val s = current) {
                    Screen.Home -> HomeScreen(tracks, vm, nav, onMenu)
                    Screen.Library -> LibraryScreen(tracks, vm, nav, onMenu)
                    Screen.Search -> SearchScreen(tracks, vm, nav, onMenu)
                    Screen.Settings -> SettingsScreen()
                    is Screen.Album -> TrackListScreen(s.name, tracks.filter { it.album == s.name }.sortedBy { it.title }, vm, { stack.removeAt(stack.lastIndex) }, onMenu)
                    is Screen.Artist -> TrackListScreen(s.name, tracks.filter { it.artist == s.name }.sortedBy { it.title }, vm, { stack.removeAt(stack.lastIndex) }, onMenu)
                    is Screen.PlaylistS -> PlaylistScreen(s.id, tracks, vm, { stack.removeAt(stack.lastIndex) }, onMenu)
                    Screen.Favorites -> TrackListScreen("Любимые", tracks.filter { it.favorite }, vm, { stack.removeAt(stack.lastIndex) }, onMenu)
                    Screen.Recent -> TrackListScreen("Недавно добавленные", tracks.sortedByDescending { it.addedAt }, vm, { stack.removeAt(stack.lastIndex) }, onMenu)
                }
            }
        }

        AnimatedVisibility(
            visible = playerOpen && current_track != null,
            enter = slideInVertically { it }, exit = slideOutVertically { it }
        ) {
            current_track?.let { FullPlayer(it, ui, tracks, vm, { playerOpen = false }, { menuTrack = it }) }
        }

        menuTrack?.let { t -> TrackMenu(t, vm, { menuTrack = null }, { stack.add(it); playerOpen = false }) }
    }
}

// ---------------- common pieces ----------------
@Composable
fun Artwork(track: Track?, size: androidx.compose.ui.unit.Dp, radius: Int = 10) {
    Box(
        Modifier.size(size).clip(RoundedCornerShape(radius.dp))
            .background(Brush.linearGradient(listOf(BlueDeep, Surface2))),
        contentAlignment = Alignment.Center
    ) {
        if (track?.artPath != null) {
            AsyncImage(model = track.artUri, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        } else {
            Icon(Icons.Default.MusicNote, null, tint = Blue.copy(alpha = .8f), modifier = Modifier.size(size * 0.45f))
        }
    }
}

@Composable
fun TrackRow(t: Track, playing: Boolean, onClick: () -> Unit, onMenu: () -> Unit, trailing: (@Composable () -> Unit)? = null) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Artwork(t, 50.dp, 8)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(t.title, color = if (playing) Blue else TextHi, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Row(verticalAlignment = Alignment.CenterVertically) {
                if (t.favorite) { Icon(Icons.Default.Favorite, null, tint = Blue, modifier = Modifier.size(12.dp)); Spacer(Modifier.width(4.dp)) }
                Text("${t.artist} • ${fmt(t.durationMs)}", color = TextLo, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        trailing?.invoke()
        IconButton(onClick = onMenu) { Icon(Icons.Default.MoreVert, null, tint = TextLo) }
    }
}

@Composable
fun SectionTitle(text: String, action: String? = null, onAction: () -> Unit = {}) {
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 20.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text, fontSize = 20.sp, fontWeight = FontWeight.Bold, color = TextHi, modifier = Modifier.weight(1f))
        if (action != null) TextButton(onClick = onAction) { Text(action, color = Blue) }
    }
}

@Composable
fun EmptyState(text: String) {
    Box(Modifier.fillMaxWidth().padding(40.dp), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(Icons.Default.LibraryMusic, null, tint = Surface2, modifier = Modifier.size(72.dp))
            Spacer(Modifier.height(12.dp))
            Text(text, color = TextLo, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
        }
    }
}

@Composable
fun ScanBanner() {
    val scan by Library.scan.collectAsStateWithLifecycle()
    if (scan.running) {
        Column(Modifier.fillMaxWidth().padding(16.dp).clip(RoundedCornerShape(14.dp)).background(Surface2).padding(14.dp)) {
            Text("Загрузка треков в медиатеку: ${scan.done} / ${scan.total}", color = TextHi, fontSize = 14.sp)
            Spacer(Modifier.height(8.dp))
            LinearProgressIndicator(
                progress = { if (scan.total == 0) 0f else scan.done / scan.total.toFloat() },
                modifier = Modifier.fillMaxWidth().clip(CircleShape), color = Blue, trackColor = Bg
            )
        }
    }
}

// ---------------- home ----------------
@Composable
fun HomeScreen(tracks: List<Track>, vm: PlayerViewModel, nav: (Screen) -> Unit, onMenu: (Track) -> Unit) {
    val ui by vm.ui.collectAsStateWithLifecycle()
    val picker = rememberImportLauncher()
    val recentPlayed = remember(tracks) { tracks.filter { it.lastPlayed > 0 }.sortedByDescending { it.lastPlayed }.take(12) }
    val recentAdded = remember(tracks) { tracks.sortedByDescending { it.addedAt }.take(12) }
    val top = remember(tracks) { tracks.filter { it.playCount > 0 }.sortedByDescending { it.playCount }.take(5) }
    LazyColumn(Modifier.fillMaxSize()) {
        item {
            Box(
                Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(BlueDeep.copy(alpha = .6f), Bg)))
                    .statusBarsPadding().padding(16.dp)
            ) {
                Column {
                    Text("BlueWave", fontSize = 30.sp, fontWeight = FontWeight.Black, color = TextHi)
                    Text("${tracks.size} треков в медиатеке", color = TextLo)
                    Spacer(Modifier.height(16.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        Button(
                            onClick = { vm.play(tracks, shuffle = true) }, enabled = tracks.isNotEmpty(),
                            colors = ButtonDefaults.buttonColors(containerColor = Blue), shape = RoundedCornerShape(24.dp)
                        ) { Icon(Icons.Default.Shuffle, null); Spacer(Modifier.width(6.dp)); Text("Перемешать всё") }
                        OutlinedButton(
                            onClick = { picker() }, shape = RoundedCornerShape(24.dp),
                            border = ButtonDefaults.outlinedButtonBorder(true).copy(brush = Brush.linearGradient(listOf(Blue, Blue)))
                        ) { Icon(Icons.Default.Add, null, tint = Blue); Spacer(Modifier.width(6.dp)); Text("Добавить", color = TextHi) }
                    }
                }
            }
        }
        item { ScanBanner() }
        item {
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                QuickTile("Любимые", Icons.Default.Favorite, Modifier.weight(1f)) { nav(Screen.Favorites) }
                QuickTile("Новое", Icons.Default.NewReleases, Modifier.weight(1f)) { nav(Screen.Recent) }
            }
        }
        if (tracks.isEmpty()) item { EmptyState("Медиатека пуста.\nДай доступ к файлам в настройках или нажми «Добавить».") }
        if (recentPlayed.isNotEmpty()) {
            item { SectionTitle("Недавно слушал") }
            item { CardRow(recentPlayed, ui.currentId) { vm.play(recentPlayed, recentPlayed.indexOf(it)) } }
        }
        if (recentAdded.isNotEmpty()) {
            item { SectionTitle("Недавно добавлено", "Все") { nav(Screen.Recent) } }
            item { CardRow(recentAdded, ui.currentId) { vm.play(recentAdded, recentAdded.indexOf(it)) } }
        }
        if (top.isNotEmpty()) {
            item { SectionTitle("Часто слушаешь") }
            items(top, key = { "top" + it.id }) { t -> TrackRow(t, t.id == ui.currentId, { vm.play(top, top.indexOf(t)) }, { onMenu(t) }) }
        }
        item { Spacer(Modifier.height(24.dp)) }
    }
}

@Composable
fun QuickTile(title: String, icon: ImageVector, modifier: Modifier, onClick: () -> Unit) {
    Row(
        modifier.clip(RoundedCornerShape(12.dp)).background(Surface2).clickable(onClick = onClick).padding(14.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(icon, null, tint = Blue); Spacer(Modifier.width(10.dp))
        Text(title, color = TextHi, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
fun CardRow(list: List<Track>, currentId: String?, onClick: (Track) -> Unit) {
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        items(list, key = { it.id }) { t ->
            Column(Modifier.width(140.dp).clickable { onClick(t) }) {
                Artwork(t, 140.dp, 14)
                Spacer(Modifier.height(6.dp))
                Text(t.title, color = if (t.id == currentId) Blue else TextHi, maxLines = 1, overflow = TextOverflow.Ellipsis, fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
                Text(t.artist, color = TextLo, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 12.sp)
            }
        }
    }
}

@Composable
fun rememberImportLauncher(): () -> Unit {
    val scope = rememberCoroutineScope()
    val l = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) scope.launch { Library.importUris(uris) }
    }
    return { l.launch(arrayOf("audio/*")) }
}

// ---------------- library ----------------
enum class Sort(val label: String) { ADDED("По дате"), TITLE("По названию"), ARTIST("По исполнителю"), PLAYS("По прослушиваниям") }

@Composable
fun LibraryScreen(tracks: List<Track>, vm: PlayerViewModel, nav: (Screen) -> Unit, onMenu: (Track) -> Unit) {
    var tab by rememberSaveable { mutableIntStateOf(0) }
    val ui by vm.ui.collectAsStateWithLifecycle()
    val playlists by Library.playlists.collectAsStateWithLifecycle()
    var sort by remember { mutableStateOf(Sort.ADDED) }
    var sortMenu by remember { mutableStateOf(false) }
    var newPl by remember { mutableStateOf(false) }
    val picker = rememberImportLauncher()
    val scope = rememberCoroutineScope()
    val sorted = remember(tracks, sort) {
        when (sort) {
            Sort.ADDED -> tracks.sortedByDescending { it.addedAt }
            Sort.TITLE -> tracks.sortedBy { it.title.lowercase() }
            Sort.ARTIST -> tracks.sortedBy { it.artist.lowercase() }
            Sort.PLAYS -> tracks.sortedByDescending { it.playCount }
        }
    }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Row(Modifier.fillMaxWidth().padding(16.dp, 12.dp, 8.dp, 0.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Медиатека", fontSize = 28.sp, fontWeight = FontWeight.Black, modifier = Modifier.weight(1f))
            IconButton(onClick = { scope.launch { Library.scanDevice() } }) { Icon(Icons.Default.Refresh, null, tint = TextHi) }
            IconButton(onClick = picker) { Icon(Icons.Default.Add, null, tint = TextHi) }
        }
        ScanBanner()
        val tabs = listOf("Треки", "Альбомы", "Исполнители", "Плейлисты")
        ScrollableTabRow(selectedTabIndex = tab, containerColor = Bg, contentColor = Blue, edgePadding = 16.dp, divider = {}) {
            tabs.forEachIndexed { i, s ->
                Tab(selected = tab == i, onClick = { tab = i }, text = { Text(s, color = if (tab == i) TextHi else TextLo, fontWeight = FontWeight.SemiBold) })
            }
        }
        when (tab) {
            0 -> LazyColumn(Modifier.fillMaxSize()) {
                item {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        FilledIconButton(onClick = { vm.play(sorted) }, colors = IconButtonDefaults.filledIconButtonColors(containerColor = Blue)) { Icon(Icons.Default.PlayArrow, null) }
                        Spacer(Modifier.width(8.dp))
                        FilledIconButton(onClick = { vm.play(sorted, shuffle = true) }, colors = IconButtonDefaults.filledIconButtonColors(containerColor = Surface2)) { Icon(Icons.Default.Shuffle, null, tint = TextHi) }
                        Spacer(Modifier.weight(1f))
                        Box {
                            TextButton(onClick = { sortMenu = true }) { Icon(Icons.Default.Sort, null, tint = TextLo); Spacer(Modifier.width(4.dp)); Text(sort.label, color = TextLo) }
                            DropdownMenu(sortMenu, { sortMenu = false }, containerColor = Surface2) {
                                Sort.entries.forEach { s -> DropdownMenuItem(text = { Text(s.label) }, onClick = { sort = s; sortMenu = false }) }
                            }
                        }
                    }
                }
                if (sorted.isEmpty()) item { EmptyState("Пока нет треков") }
                itemsIndexed(sorted, key = { _, t -> t.id }) { i, t -> TrackRow(t, t.id == ui.currentId, { vm.play(sorted, i) }, { onMenu(t) }) }
            }
            1 -> GroupGrid(tracks.groupBy { it.album }, { it.firstOrNull { t -> t.artPath != null } ?: it.first() }, { k, l -> "${l.first().artist} • ${l.size}" }) { nav(Screen.Album(it)) }
            2 -> LazyColumn(Modifier.fillMaxSize()) {
                val groups = tracks.groupBy { it.artist }.toSortedMap(String.CASE_INSENSITIVE_ORDER)
                items(groups.keys.toList(), key = { it }) { a ->
                    val l = groups[a]!!
                    Row(Modifier.fillMaxWidth().clickable { nav(Screen.Artist(a)) }.padding(16.dp, 8.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(56.dp).clip(CircleShape)) { Artwork(l.firstOrNull { it.artPath != null } ?: l.first(), 56.dp, 28) }
                        Spacer(Modifier.width(14.dp))
                        Column { Text(a, fontWeight = FontWeight.SemiBold); Text("${l.size} треков", color = TextLo, fontSize = 13.sp) }
                    }
                }
            }
            else -> LazyColumn(Modifier.fillMaxSize()) {
                item {
                    Row(Modifier.fillMaxWidth().clickable { newPl = true }.padding(16.dp, 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(56.dp).clip(RoundedCornerShape(10.dp)).background(Surface2), contentAlignment = Alignment.Center) { Icon(Icons.Default.Add, null, tint = Blue) }
                        Spacer(Modifier.width(14.dp)); Text("Новый плейлист", fontWeight = FontWeight.SemiBold)
                    }
                }
                item {
                    Row(Modifier.fillMaxWidth().clickable { nav(Screen.Favorites) }.padding(16.dp, 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Box(Modifier.size(56.dp).clip(RoundedCornerShape(10.dp)).background(Brush.linearGradient(listOf(Blue, BlueDeep))), contentAlignment = Alignment.Center) { Icon(Icons.Default.Favorite, null, tint = Color.White) }
                        Spacer(Modifier.width(14.dp))
                        Column { Text("Любимые", fontWeight = FontWeight.SemiBold); Text("${tracks.count { it.favorite }} треков", color = TextLo, fontSize = 13.sp) }
                    }
                }
                items(playlists, key = { it.id }) { p ->
                    val first = p.trackIds.firstNotNullOfOrNull { id -> tracks.firstOrNull { it.id == id } }
                    Row(Modifier.fillMaxWidth().clickable { nav(Screen.PlaylistS(p.id)) }.padding(16.dp, 10.dp), verticalAlignment = Alignment.CenterVertically) {
                        Artwork(first, 56.dp)
                        Spacer(Modifier.width(14.dp))
                        Column { Text(p.name, fontWeight = FontWeight.SemiBold); Text("Плейлист • ${p.trackIds.size} треков", color = TextLo, fontSize = 13.sp) }
                    }
                }
            }
        }
    }
    if (newPl) NameDialog("Новый плейлист", "") { name -> newPl = false; if (name != null && name.isNotBlank()) scope.launch { Library.createPlaylist(name.trim()) } }
}

@Composable
fun GroupGrid(groups: Map<String, List<Track>>, cover: (List<Track>) -> Track, sub: (String, List<Track>) -> String, onClick: (String) -> Unit) {
    val keys = remember(groups) { groups.keys.sortedBy { it.lowercase() } }
    LazyVerticalGrid(GridCells.Adaptive(150.dp), contentPadding = PaddingValues(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        items(keys, key = { it }) { k ->
            val l = groups[k]!!
            Column(Modifier.clickable { onClick(k) }) {
                BoxWithConstraints { Artwork(cover(l), maxWidth, 14) }
                Spacer(Modifier.height(6.dp))
                Text(k, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(sub(k, l), color = TextLo, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

// ---------------- detail lists ----------------
@Composable
fun DetailHeader(title: String, subtitle: String, cover: Track?, onBack: () -> Unit, actions: @Composable RowScope.() -> Unit = {}) {
    Column(
        Modifier.fillMaxWidth().background(Brush.verticalGradient(listOf(BlueDeep.copy(alpha = .7f), Bg))).statusBarsPadding().padding(bottom = 8.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, null, tint = TextHi) }
            Spacer(Modifier.weight(1f)); actions()
        }
        Row(Modifier.padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
            Artwork(cover, 110.dp, 12)
            Spacer(Modifier.width(16.dp))
            Column {
                Text(title, fontSize = 24.sp, fontWeight = FontWeight.Black, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(subtitle, color = TextLo)
            }
        }
    }
}

@Composable
fun PlayButtons(list: List<Track>, vm: PlayerViewModel) {
    Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Button(onClick = { vm.play(list) }, enabled = list.isNotEmpty(), colors = ButtonDefaults.buttonColors(containerColor = Blue), shape = RoundedCornerShape(24.dp)) {
            Icon(Icons.Default.PlayArrow, null); Spacer(Modifier.width(4.dp)); Text("Слушать")
        }
        Button(onClick = { vm.play(list, shuffle = true) }, enabled = list.isNotEmpty(), colors = ButtonDefaults.buttonColors(containerColor = Surface2), shape = RoundedCornerShape(24.dp)) {
            Icon(Icons.Default.Shuffle, null, tint = TextHi); Spacer(Modifier.width(4.dp)); Text("Перемешать", color = TextHi)
        }
    }
}

@Composable
fun TrackListScreen(title: String, list: List<Track>, vm: PlayerViewModel, onBack: () -> Unit, onMenu: (Track) -> Unit) {
    val ui by vm.ui.collectAsStateWithLifecycle()
    LazyColumn(Modifier.fillMaxSize()) {
        item { DetailHeader(title, "${list.size} треков • ${fmt(list.sumOf { it.durationMs })}", list.firstOrNull { it.artPath != null } ?: list.firstOrNull(), onBack) }
        item { PlayButtons(list, vm) }
        if (list.isEmpty()) item { EmptyState("Здесь пока пусто") }
        itemsIndexed(list, key = { _, t -> t.id }) { i, t -> TrackRow(t, t.id == ui.currentId, { vm.play(list, i) }, { onMenu(t) }) }
    }
}

@Composable
fun PlaylistScreen(id: String, tracks: List<Track>, vm: PlayerViewModel, onBack: () -> Unit, onMenu: (Track) -> Unit) {
    val playlists by Library.playlists.collectAsStateWithLifecycle()
    val ui by vm.ui.collectAsStateWithLifecycle()
    val p = playlists.firstOrNull { it.id == id } ?: run { LaunchedEffect(Unit) { onBack() }; return }
    val list = p.trackIds.mapNotNull { tid -> tracks.firstOrNull { it.id == tid } }
    val scope = rememberCoroutineScope()
    var rename by remember { mutableStateOf(false) }
    var confirmDel by remember { mutableStateOf(false) }
    LazyColumn(Modifier.fillMaxSize()) {
        item {
            DetailHeader(p.name, "Плейлист • ${list.size} треков", list.firstOrNull(), onBack) {
                IconButton(onClick = { rename = true }) { Icon(Icons.Default.Edit, null, tint = TextHi) }
                IconButton(onClick = { confirmDel = true }) { Icon(Icons.Default.Delete, null, tint = TextHi) }
            }
        }
        item { PlayButtons(list, vm) }
        if (list.isEmpty()) item { EmptyState("Добавляй треки через меню «⋮» → «В плейлист»") }
        itemsIndexed(list, key = { _, t -> t.id }) { i, t ->
            TrackRow(t, t.id == ui.currentId, { vm.play(list, i) }, { onMenu(t) }) {
                IconButton(onClick = { scope.launch { Library.removeFromPlaylist(p.id, t.id) } }) { Icon(Icons.Default.RemoveCircleOutline, null, tint = TextLo) }
            }
        }
    }
    if (rename) NameDialog("Переименовать", p.name) { n -> rename = false; if (!n.isNullOrBlank()) scope.launch { Library.renamePlaylist(p.id, n.trim()) } }
    if (confirmDel) AlertDialog(
        onDismissRequest = { confirmDel = false }, containerColor = Surface2,
        title = { Text("Удалить плейлист?") }, text = { Text("Треки останутся в медиатеке.") },
        confirmButton = { TextButton(onClick = { confirmDel = false; scope.launch { Library.deletePlaylist(p.id) } }) { Text("Удалить", color = Blue) } },
        dismissButton = { TextButton(onClick = { confirmDel = false }) { Text("Отмена", color = TextLo) } }
    )
}

@Composable
fun NameDialog(title: String, initial: String, onResult: (String?) -> Unit) {
    var text by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = { onResult(null) }, containerColor = Surface2, title = { Text(title) },
        text = { OutlinedTextField(text, { text = it }, singleLine = true, placeholder = { Text("Название") }) },
        confirmButton = { TextButton(onClick = { onResult(text) }) { Text("OK", color = Blue) } },
        dismissButton = { TextButton(onClick = { onResult(null) }) { Text("Отмена", color = TextLo) } }
    )
}

// ---------------- search ----------------
@Composable
fun SearchScreen(tracks: List<Track>, vm: PlayerViewModel, nav: (Screen) -> Unit, onMenu: (Track) -> Unit) {
    var q by rememberSaveable { mutableStateOf("") }
    val ui by vm.ui.collectAsStateWithLifecycle()
    val ql = q.trim().lowercase()
    val res = remember(tracks, ql) {
        if (ql.isEmpty()) emptyList() else tracks.filter { ql in it.title.lowercase() || ql in it.artist.lowercase() || ql in it.album.lowercase() }
    }
    val artists = remember(res) { res.map { it.artist }.distinct().filter { ql in it.lowercase() }.take(5) }
    val albums = remember(res) { res.map { it.album }.distinct().filter { ql in it.lowercase() }.take(5) }
    Column(Modifier.fillMaxSize().statusBarsPadding()) {
        Text("Поиск", fontSize = 28.sp, fontWeight = FontWeight.Black, modifier = Modifier.padding(16.dp, 12.dp))
        TextField(
            q, { q = it }, singleLine = true,
            placeholder = { Text("Треки, исполнители, альбомы") },
            leadingIcon = { Icon(Icons.Default.Search, null) },
            trailingIcon = { if (q.isNotEmpty()) IconButton(onClick = { q = "" }) { Icon(Icons.Default.Close, null) } },
            shape = RoundedCornerShape(14.dp),
            colors = TextFieldDefaults.colors(
                focusedContainerColor = Surface2, unfocusedContainerColor = Surface2,
                focusedIndicatorColor = Color.Transparent, unfocusedIndicatorColor = Color.Transparent, cursorColor = Blue
            ),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)
        )
        LazyColumn(Modifier.fillMaxSize()) {
            if (ql.isNotEmpty() && res.isEmpty()) item { EmptyState("Ничего не найдено") }
            if (artists.isNotEmpty()) {
                item { SectionTitle("Исполнители") }
                items(artists) { a -> ListItem(headlineContent = { Text(a) }, leadingContent = { Icon(Icons.Default.Person, null, tint = Blue) }, colors = ListItemDefaults.colors(containerColor = Bg), modifier = Modifier.clickable { nav(Screen.Artist(a)) }) }
            }
            if (albums.isNotEmpty()) {
                item { SectionTitle("Альбомы") }
                items(albums) { a -> ListItem(headlineContent = { Text(a) }, leadingContent = { Icon(Icons.Default.Album, null, tint = Blue) }, colors = ListItemDefaults.colors(containerColor = Bg), modifier = Modifier.clickable { nav(Screen.Album(a)) }) }
            }
            if (res.isNotEmpty()) {
                item { SectionTitle("Треки") }
                itemsIndexed(res, key = { _, t -> t.id }) { i, t -> TrackRow(t, t.id == ui.currentId, { vm.play(res, i) }, { onMenu(t) }) }
            }
        }
    }
}

// ---------------- settings ----------------
@Composable
fun SettingsScreen() {
    val ctx = LocalContext.current
    val prefs = remember { ctx.getSharedPreferences("app", Context.MODE_PRIVATE) }
    var autoscan by remember { mutableStateOf(prefs.getBoolean("autoscan", true)) }
    var granted by remember { mutableStateOf(hasAudioPerm(ctx)) }
    val tracks by Library.tracks.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val size = remember(tracks) { Library.storageBytes() }
    val permL = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        granted = it; if (it) scope.launch { Library.scanDevice() }
    }
    val picker = rememberImportLauncher()
    Column(Modifier.fillMaxSize().statusBarsPadding().padding(16.dp)) {
        Text("Настройки", fontSize = 28.sp, fontWeight = FontWeight.Black)
        Spacer(Modifier.height(16.dp))
        SettingCard {
            Text("Доступ к музыке на устройстве", fontWeight = FontWeight.SemiBold)
            Text(if (granted) "Разрешено" else "Не разрешено", color = if (granted) Blue else TextLo, fontSize = 13.sp)
            if (!granted) {
                Spacer(Modifier.height(8.dp))
                Button(onClick = { permL.launch(audioPermission()) }, colors = ButtonDefaults.buttonColors(containerColor = Blue)) { Text("Разрешить") }
            }
        }
        SettingCard {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("Автозагрузка новых треков", fontWeight = FontWeight.SemiBold)
                    Text("При каждом открытии ищет скачанные треки и копирует их в медиатеку", color = TextLo, fontSize = 13.sp)
                }
                Switch(autoscan, { autoscan = it; prefs.edit().putBoolean("autoscan", it).apply() },
                    colors = SwitchDefaults.colors(checkedTrackColor = Blue, checkedThumbColor = Color.White))
            }
        }
        SettingCard {
            Text("Медиатека", fontWeight = FontWeight.SemiBold)
            Text("${tracks.size} треков • ${"%.1f".format(size / 1048576.0)} МБ во внутренней памяти приложения", color = TextLo, fontSize = 13.sp)
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = { scope.launch { Library.scanDevice() } }, enabled = granted, colors = ButtonDefaults.buttonColors(containerColor = Blue)) { Text("Сканировать") }
                Button(onClick = picker, colors = ButtonDefaults.buttonColors(containerColor = Bg)) { Text("Выбрать файлы", color = TextHi) }
            }
        }
        ScanBanner()
        SettingCard {
            Text("Как это работает", fontWeight = FontWeight.SemiBold)
            Text("Треки копируются в закрытое хранилище приложения: их не видно в файловом менеджере, и они продолжают играть, даже если удалить оригиналы с телефона. Удалённый из приложения трек не будет загружен заново при сканировании.", color = TextLo, fontSize = 13.sp)
        }
    }
}

@Composable
fun SettingCard(content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp).clip(RoundedCornerShape(16.dp)).background(Surface1).border(1.dp, Surface2, RoundedCornerShape(16.dp)).padding(16.dp), content = content)
}

// ---------------- mini & full player ----------------
@Composable
fun MiniPlayer(t: Track, ui: PlayerUi, vm: PlayerViewModel, onOpen: () -> Unit) {
    Column(Modifier.padding(horizontal = 8.dp).clip(RoundedCornerShape(12.dp)).background(Surface2).clickable(onClick = onOpen)) {
        Row(Modifier.padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Artwork(t, 44.dp, 8)
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(t.title, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 14.sp)
                Text(t.artist, color = TextLo, maxLines = 1, overflow = TextOverflow.Ellipsis, fontSize = 12.sp)
            }
            IconButton(onClick = { vm.toggle() }) { Icon(if (ui.isPlaying) Icons.Default.Pause else Icons.Default.PlayArrow, null, tint = TextHi) }
            IconButton(onClick = { vm.next() }) { Icon(Icons.Default.SkipNext, null, tint = TextHi) }
        }
        LinearProgressIndicator(
            progress = { if (ui.duration > 0) (ui.position / ui.duration.toFloat()).coerceIn(0f, 1f) else 0f },
            modifier = Modifier.fillMaxWidth().height(2.dp), color = Blue, trackColor = Surface1, drawStopIndicator = {}
        )
    }
    Spacer(Modifier.height(4.dp))
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun FullPlayer(t: Track, ui: PlayerUi, tracks: List<Track>, vm: PlayerViewModel, onClose: () -> Unit, onMenu: (Track) -> Unit) {
    val scope = rememberCoroutineScope()
    var seeking by remember { mutableStateOf<Float?>(null) }
    var showQueue by remember { mutableStateOf(false) }
    var showSleep by remember { mutableStateOf(false) }
    var showSpeed by remember { mutableStateOf(false) }
    Box(
        Modifier.fillMaxSize().background(Brush.verticalGradient(listOf(BlueDeep, Color(0xFF070A12), Bg)))
            .clickable(enabled = false) {}
    ) {
        Column(Modifier.fillMaxSize().systemBarsPadding().padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose) { Icon(Icons.Default.KeyboardArrowDown, null, tint = TextHi, modifier = Modifier.size(32.dp)) }
                Column(Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("СЕЙЧАС ИГРАЕТ", color = TextLo, fontSize = 11.sp, letterSpacing = 2.sp)
                    Text(t.album, color = TextHi, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                IconButton(onClick = { onMenu(t) }) { Icon(Icons.Default.MoreVert, null, tint = TextHi) }
            }
            Spacer(Modifier.weight(1f))
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                Artwork(t, maxWidth, 20)
            }
            Spacer(Modifier.weight(1f))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(t.title, fontSize = 24.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(t.artist, color = TextLo, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                IconButton(onClick = { scope.launch { Library.toggleFavorite(t.id) } }) {
                    Icon(if (t.favorite) Icons.Default.Favorite else Icons.Default.FavoriteBorder, null, tint = if (t.favorite) Blue else TextHi, modifier = Modifier.size(28.dp))
                }
            }
            Spacer(Modifier.height(12.dp))
            val dur = ui.duration.coerceAtLeast(1)
            Slider(
                value = seeking ?: (ui.position / dur.toFloat()).coerceIn(0f, 1f),
                onValueChange = { seeking = it },
                onValueChangeFinished = { seeking?.let { vm.seek((it * dur).toLong()) }; seeking = null },
                colors = SliderDefaults.colors(thumbColor = Color.White, activeTrackColor = Blue, inactiveTrackColor = Color.White.copy(alpha = .15f))
            )
            Row(Modifier.fillMaxWidth()) {
                Text(fmt(seeking?.let { (it * dur).toLong() } ?: ui.position), color = TextLo, fontSize = 12.sp)
                Spacer(Modifier.weight(1f))
                Text(fmt(ui.duration), color = TextLo, fontSize = 12.sp)
            }
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = { vm.toggleShuffle() }) { Icon(Icons.Default.Shuffle, null, tint = if (ui.shuffle) Blue else TextHi) }
                IconButton(onClick = { vm.prev() }, modifier = Modifier.size(56.dp)) { Icon(Icons.Default.SkipPrevious, null, tint = TextHi, modifier = Modifier.size(40.dp)) }
                Box(
                    Modifier.size(76.dp).clip(CircleShape).background(Color.White).clickable { vm.toggle() },
                    contentAlignment = Alignment.Center
                ) { Icon(if (ui.isPlaying) Icons.Default.Pause else Icons.Default.PlayArrow, null, tint = Bg, modifier = Modifier.size(42.dp)) }
                IconButton(onClick = { vm.next() }, modifier = Modifier.size(56.dp)) { Icon(Icons.Default.SkipNext, null, tint = TextHi, modifier = Modifier.size(40.dp)) }
                IconButton(onClick = { vm.cycleRepeat() }) {
                    Icon(if (ui.repeat == Player.REPEAT_MODE_ONE) Icons.Default.RepeatOne else Icons.Default.Repeat, null,
                        tint = if (ui.repeat != Player.REPEAT_MODE_OFF) Blue else TextHi)
                }
            }
            Spacer(Modifier.height(16.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                TextButton(onClick = { showSpeed = true }) { Icon(Icons.Default.Speed, null, tint = if (ui.speed != 1f) Blue else TextLo); Spacer(Modifier.width(4.dp)); Text("${ui.speed}x", color = TextLo) }
                TextButton(onClick = { showSleep = true }) {
                    Icon(Icons.Default.Bedtime, null, tint = if (ui.sleepAt > 0) Blue else TextLo); Spacer(Modifier.width(4.dp))
                    Text(if (ui.sleepAt > 0) "до ${java.text.SimpleDateFormat("HH:mm").format(ui.sleepAt)}" else "Таймер", color = TextLo)
                }
                TextButton(onClick = { showQueue = true }) { Icon(Icons.AutoMirrored.Filled.QueueMusic, null, tint = TextLo); Spacer(Modifier.width(4.dp)); Text("Очередь", color = TextLo) }
            }
            Spacer(Modifier.height(12.dp))
        }
    }
    if (showQueue) ModalBottomSheet(onDismissRequest = { showQueue = false }, containerColor = Surface1) {
        Text("Очередь • ${ui.queue.size}", fontSize = 20.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(16.dp, 0.dp, 16.dp, 8.dp))
        LazyColumn(Modifier.fillMaxHeight(.7f)) {
            itemsIndexed(ui.queue) { i, id ->
                val qt = tracks.firstOrNull { it.id == id } ?: return@itemsIndexed
                Row(Modifier.fillMaxWidth().clickable { vm.jumpTo(i) }.padding(16.dp, 6.dp), verticalAlignment = Alignment.CenterVertically) {
                    Artwork(qt, 42.dp, 6); Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(qt.title, color = if (i == ui.index) Blue else TextHi, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(qt.artist, color = TextLo, fontSize = 12.sp, maxLines = 1)
                    }
                    if (i > 0) IconButton(onClick = { vm.moveInQueue(i, i - 1) }) { Icon(Icons.Default.KeyboardArrowUp, null, tint = TextLo) }
                    if (i != ui.index) IconButton(onClick = { vm.removeFromQueue(i) }) { Icon(Icons.Default.Close, null, tint = TextLo) }
                }
            }
        }
    }
    if (showSleep) AlertDialog(
        onDismissRequest = { showSleep = false }, containerColor = Surface2, title = { Text("Таймер сна") },
        text = {
            Column {
                listOf(0 to "Выключить", 10 to "10 минут", 15 to "15 минут", 30 to "30 минут", 45 to "45 минут", 60 to "1 час", 90 to "1,5 часа").forEach { (m, l) ->
                    Text(l, modifier = Modifier.fillMaxWidth().clickable { vm.sleepTimer(m); showSleep = false }.padding(vertical = 12.dp))
                }
            }
        }, confirmButton = {}
    )
    if (showSpeed) AlertDialog(
        onDismissRequest = { showSpeed = false }, containerColor = Surface2, title = { Text("Скорость") },
        text = {
            Column {
                listOf(0.5f, 0.75f, 1f, 1.25f, 1.5f, 2f).forEach { s ->
                    Text("${s}x", color = if (s == ui.speed) Blue else TextHi, modifier = Modifier.fillMaxWidth().clickable { vm.setSpeed(s); showSpeed = false }.padding(vertical = 12.dp))
                }
            }
        }, confirmButton = {}
    )
}


// ---------------- track menu ----------------
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TrackMenu(t: Track, vm: PlayerViewModel, onDismiss: () -> Unit, nav: (Screen) -> Unit) {
    val scope = rememberCoroutineScope()
    val playlists by Library.playlists.collectAsStateWithLifecycle()
    val live by Library.tracks.collectAsStateWithLifecycle()
    val tr = live.firstOrNull { it.id == t.id } ?: t
    var pickPl by remember { mutableStateOf(false) }
    var newPl by remember { mutableStateOf(false) }
    var edit by remember { mutableStateOf(false) }
    var confirmDel by remember { mutableStateOf(false) }
    ModalBottomSheet(onDismissRequest = onDismiss, containerColor = Surface1) {
        Row(Modifier.padding(16.dp, 0.dp, 16.dp, 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Artwork(tr, 56.dp); Spacer(Modifier.width(12.dp))
            Column { Text(tr.title, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis); Text(tr.artist, color = TextLo, maxLines = 1) }
        }
        HorizontalDivider(color = Surface2)
        @Composable fun item(icon: ImageVector, label: String, onClick: () -> Unit) =
            ListItem(headlineContent = { Text(label) }, leadingContent = { Icon(icon, null, tint = TextHi) },
                colors = ListItemDefaults.colors(containerColor = Surface1), modifier = Modifier.clickable(onClick = onClick))
        if (!pickPl) {
            item(Icons.AutoMirrored.Filled.PlaylistPlay, "Играть следующим") { vm.playNext(tr); onDismiss() }
            item(Icons.AutoMirrored.Filled.QueueMusic, "Добавить в очередь") { vm.addToQueue(tr); onDismiss() }
            item(if (tr.favorite) Icons.Default.Favorite else Icons.Default.FavoriteBorder, if (tr.favorite) "Убрать из любимых" else "В любимые") { scope.launch { Library.toggleFavorite(tr.id) } }
            item(Icons.AutoMirrored.Filled.PlaylistAdd, "В плейлист") { pickPl = true }
            item(Icons.Default.Person, "Исполнитель: ${tr.artist}") { onDismiss(); nav(Screen.Artist(tr.artist)) }
            item(Icons.Default.Album, "Альбом: ${tr.album}") { onDismiss(); nav(Screen.Album(tr.album)) }
            item(Icons.Default.Edit, "Изменить информацию") { edit = true }
            item(Icons.Default.Delete, "Удалить из приложения") { confirmDel = true }
        } else {
            item(Icons.Default.Add, "Новый плейлист") { newPl = true }
            playlists.forEach { p -> item(Icons.AutoMirrored.Filled.QueueMusic, p.name) { scope.launch { Library.addToPlaylist(p.id, tr.id) }; onDismiss() } }
        }
        Spacer(Modifier.height(24.dp))
    }
    if (newPl) NameDialog("Новый плейлист", "") { n ->
        newPl = false
        if (!n.isNullOrBlank()) scope.launch { val p = Library.createPlaylist(n.trim()); Library.addToPlaylist(p.id, tr.id); onDismiss() }
    }
    if (edit) {
        var title by remember { mutableStateOf(tr.title) }
        var artist by remember { mutableStateOf(tr.artist) }
        var album by remember { mutableStateOf(tr.album) }
        AlertDialog(
            onDismissRequest = { edit = false }, containerColor = Surface2, title = { Text("Информация о треке") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(title, { title = it }, label = { Text("Название") }, singleLine = true)
                    OutlinedTextField(artist, { artist = it }, label = { Text("Исполнитель") }, singleLine = true)
                    OutlinedTextField(album, { album = it }, label = { Text("Альбом") }, singleLine = true)
                }
            },
            confirmButton = { TextButton(onClick = { edit = false; scope.launch { Library.editTrack(tr.id, title.trim(), artist.trim(), album.trim()) } }) { Text("Сохранить", color = Blue) } },
            dismissButton = { TextButton(onClick = { edit = false }) { Text("Отмена", color = TextLo) } }
        )
    }
    if (confirmDel) AlertDialog(
        onDismissRequest = { confirmDel = false }, containerColor = Surface2,
        title = { Text("Удалить трек?") }, text = { Text("Трек будет удалён из медиатеки приложения и не загрузится повторно при сканировании.") },
        confirmButton = { TextButton(onClick = { confirmDel = false; vm.onTrackDeleted(tr.id); scope.launch { Library.deleteTrack(tr.id) }; onDismiss() }) { Text("Удалить", color = Blue) } },
        dismissButton = { TextButton(onClick = { confirmDel = false }) { Text("Отмена", color = TextLo) } }
    )
}
