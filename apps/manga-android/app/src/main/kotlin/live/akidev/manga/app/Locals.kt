package live.akidev.manga.app

import androidx.compose.runtime.compositionLocalOf

/** The app's shared state and its stack of pushed screens, available to every screen without passing them down. */
val LocalApp = compositionLocalOf<AppModel> { error("No AppModel") }
val LocalRouter = compositionLocalOf<Router> { error("No Router") }
