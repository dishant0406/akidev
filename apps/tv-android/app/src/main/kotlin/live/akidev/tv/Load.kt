package live.akidev.tv

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import android.os.SystemClock
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.akitv.TvException

/** What a screen shows while it waits for the engine. */
sealed interface Load<out T> {
    data object Loading : Load<Nothing>
    data class Failed(val reason: String) : Load<Nothing>
    data class Done<T>(val value: T) : Load<T>
}

/** Runs `block` (a call into the Rust engine) off the main thread. It runs again when a key changes. `what` names it in the timing log. */
@Composable
fun <T> rememberLoad(what: String, vararg keys: Any?, block: () -> T): State<Load<T>> {
    val state = remember(*keys) { mutableStateOf<Load<T>>(Load.Loading) }
    LaunchedEffect(*keys) {
        state.value = Load.Loading
        val started = SystemClock.elapsedRealtime()
        state.value = withContext(Dispatchers.IO) {
            try {
                Load.Done(block()).also { Perf.log("load $what", SystemClock.elapsedRealtime() - started) }
            } catch (e: TvException.Failed) {
                Load.Failed(e.reason)
            } catch (e: Exception) {
                Load.Failed(e.message ?: "Something went wrong.")
            }
        }
    }
    return state
}
