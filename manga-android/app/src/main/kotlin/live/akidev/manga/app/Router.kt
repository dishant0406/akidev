package live.akidev.manga.app

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** A screen pushed over the tabs. */
sealed interface Route {
    /** Whether the floating tab bar is hidden while this screen is shown. */
    val hidesTabBar: Boolean get() = true
}

/** A route on the stack. It stays on it, going out of sight, until the animation that takes it away has finished. */
class Pushed(val route: Route) {
    var leaving by mutableStateOf(false)
}

/** The screens pushed over the tab the person is on: the last one is shown, going back shows the one before. Screens under it keep their state. */
class Router {
    val stack = mutableStateListOf<Pushed>()

    /** The route on top, or null when the tabs are. */
    val top get() = stack.lastOrNull { !it.leaving }?.route

    fun push(route: Route) {
        stack.add(Pushed(route))
    }

    fun back() {
        stack.lastOrNull { !it.leaving }?.leaving = true
    }

    /** Called when a screen has finished leaving. */
    fun remove(pushed: Pushed) {
        stack.remove(pushed)
    }
}
