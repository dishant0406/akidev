package live.akidev.manga.app

import androidx.compose.runtime.mutableStateListOf

/** A screen pushed over the tabs. */
sealed interface Route {
    /** Whether the floating tab bar is hidden while this screen is shown. */
    val hidesTabBar: Boolean get() = true
}

/** The screens pushed over the tab the person is on: the last one is shown, going back shows the one before. */
class Router {
    val stack = mutableStateListOf<Route>()
    val top get() = stack.lastOrNull()

    fun push(route: Route) {
        stack.add(route)
    }

    fun back() {
        if (stack.isNotEmpty()) stack.removeAt(stack.lastIndex)
    }
}
