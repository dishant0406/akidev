package live.akidev.manga.manga

import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import live.akidev.manga.design.AkiChips
import live.akidev.manga.design.AkiChoice

/** The manga screens in the Manga tab, one row of choices: the library's two lists, the updates' two lists, and Discover. */
enum class MangaPart { Saved, History, Unread, Recent, Discover }

/** Switches the part of the Manga tab on screen; null on screens that are not inside the Manga tab. */
class MangaPartControl(val part: MangaPart, val choose: (MangaPart) -> Unit)

val LocalMangaPart = compositionLocalOf<MangaPartControl?> { null }

/** The pills that switch between the manga screens, under the header of whichever is showing; nothing outside the Manga tab. */
@Composable
fun MangaPartPills() {
    val control = LocalMangaPart.current ?: return
    AkiChips(
        listOf(AkiChoice(MangaPart.Saved, "Saved"), AkiChoice(MangaPart.History, "History"), AkiChoice(MangaPart.Unread, "Unread"), AkiChoice(MangaPart.Recent, "Recent"), AkiChoice(MangaPart.Discover, "Discover")),
        control.part, control.choose,
    )
}
