package live.akidev.manga.manga

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.app.LocalApp
import live.akidev.manga.app.describe
import live.akidev.manga.design.*
import uniffi.akimanga.BookInfo
import uniffi.akimanga.CategoryInfo

/** Add, rename, put in order and delete the categories: the shelves of the library. */
@Composable
fun CategoriesSheet(onDismiss: () -> Unit) {
    val app = LocalApp.current
    var categories by remember { mutableStateOf(runCatching { app.engine.categories() }.getOrDefault(emptyList())) }
    var naming by remember { mutableStateOf(false) }

    /** The category being renamed; null when the name asked for is a new one. */
    var renaming by remember { mutableStateOf<CategoryInfo?>(null) }
    var deleting by remember { mutableStateOf<CategoryInfo?>(null) }
    var problem by remember { mutableStateOf<String?>(null) }

    /** Reads the categories again and tells the library (the screen under this sheet) to do the same. */
    fun reload() {
        categories = runCatching { app.engine.categories() }.getOrDefault(emptyList())
        app.refresh()
    }

    fun move(index: Int, step: Int) {
        val ids = categories.map { it.id }.toMutableList().also { java.util.Collections.swap(it, index, index + step) }
        runCatching { app.engine.reorderCategories(ids) }
        reload()
    }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText("Categories", AkiTextStyle.Title)
            if (categories.isEmpty()) {
                AkiEmptyState("No categories yet", "A category is a shelf, for series in your library and for books. Make one, then put series or books on it.", Modifier.heightIn(min = 220.dp))
            }
            categories.forEachIndexed { index, category ->
                val items = buildList {
                    add(AkiMenuItem("Rename", "sliders") { renaming = category; naming = true })
                    if (index > 0) add(AkiMenuItem("Move up", "order") { move(index, -1) })
                    if (index < categories.size - 1) add(AkiMenuItem("Move down", "down") { move(index, 1) })
                    add(AkiMenuItem("Delete", "trash", destructive = true) { deleting = category })
                }
                AkiRow(category.name, subtitle = contents(category)) { AkiMenu("more", "Options for ${category.name}", items) }
            }
            AkiButton("Add a category", icon = "folder-add", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true) { renaming = null; naming = true }
        }
    }
    if (naming) {
        val current = renaming
        AkiPrompt(
            if (current == null) "New category" else "Rename category", "A name for a shelf, for series and for books.", "Reading", if (current == null) "Add" else "Rename",
            onDismiss = { naming = false }, initial = current?.name ?: "",
            onConfirm = { text ->
                naming = false
                val name = text.trim()
                if (name.isNotEmpty()) {
                    try {
                        if (current != null) app.engine.renameCategory(current.id, name) else app.engine.addCategory(name)
                        reload()
                    } catch (error: Exception) {
                        problem = describe(error).message
                    }
                }
            },
        )
    }
    deleting?.let { category ->
        AkiConfirm("Delete “${category.name}”?", "The series and books on it stay where they are.", "Delete", { deleting = null }) {
            runCatching { app.engine.deleteCategory(category.id) }
            if (app.prefs.saveCategory == category.id) app.prefs.saveCategory = 0
            reload()
        }
    }
    problem?.let { AkiAlert("That did not work", it) { problem = null } }
}

private fun contents(category: CategoryInfo): String {
    val parts = listOfNotNull(
        if (category.count > 0u) (if (category.count == 1u) "1 series" else "${category.count} series") else null,
        if (category.books > 0u) (if (category.books == 1u) "1 book" else "${category.books} books") else null,
    )
    return if (parts.isEmpty()) "Empty" else parts.joinToString(" · ")
}

/** A series (or several) and the categories it is on, to change: each category is a switch, on when every one of them is on it. */
@Composable
fun CategoryPicker(series: List<Pair<String, String>>, onDismiss: () -> Unit, books: List<BookInfo> = emptyList()) {
    val app = LocalApp.current
    val categories = remember { runCatching { app.engine.categories() }.getOrDefault(emptyList()) }
    var on by remember {
        val entries = app.library.filter { (it.source to it.id) in series }
        mutableStateOf(
            categories.map { it.id }.filter { id ->
                (entries.isNotEmpty() && entries.all { id in it.categories }) || (books.isNotEmpty() && books.all { id in it.categories })
            }.toSet(),
        )
    }

    fun set(category: CategoryInfo, member: Boolean) {
        for ((source, id) in series) runCatching { app.engine.setCategory(source, id, category.id, member) }
        for (book in books) runCatching { app.engine.setBookCategory(book.id, category.id, member) }
        on = if (member) on + category.id else on - category.id
        if (series.isNotEmpty()) app.refresh()
    }

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText("Categories", AkiTextStyle.Title)
            if (categories.isEmpty()) {
                AkiEmptyState("No categories yet", "Make one from the menu on the Library or Books screen, under Categories.", Modifier.heightIn(min = 220.dp))
            } else {
                AkiCard(padding = 16.dp) {
                    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                        for (category in categories) AkiSwitch(category.name, category.id in on, { set(category, it) })
                    }
                }
            }
        }
    }
}
