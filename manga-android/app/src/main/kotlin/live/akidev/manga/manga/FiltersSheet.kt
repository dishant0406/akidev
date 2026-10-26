package live.akidev.manga.manga

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import live.akidev.manga.design.*
import uniffi.akimanga.FilterInfo
import uniffi.akimanga.FilterKind

/** The ways a source lets a search be narrowed (an order, a genre, a status). */
@Composable
fun FiltersSheet(filters: List<FilterInfo>, choices: Map<String, String>, onChoices: (Map<String, String>) -> Unit, apply: () -> Unit, onDismiss: () -> Unit) {
    // A choice equal to the filter's default is not a choice.
    fun value(filter: FilterInfo) = choices[filter.id] ?: filter.default
    fun set(filter: FilterInfo, value: String) = onChoices(if (value == filter.default) choices - filter.id else choices + (filter.id to value))

    AkiSheet(onDismiss) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(24.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            AkiText("Filters", AkiTextStyle.Title)
            for (filter in filters) {
                AkiCard(padding = 16.dp) {
                    when (filter.kind) {
                        FilterKind.SELECT -> AkiSelect(filter.name, value(filter), { set(filter, it) }, filter.options.map { AkiOption(it.value, it.name) })
                        FilterKind.TEXT -> Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            AkiText(filter.name, AkiTextStyle.BodyStrong)
                            AkiInput(filter.name, value(filter), { set(filter, it) })
                        }
                    }
                }
            }
            AkiButton("Show results", variant = AkiButtonVariant.Primary, size = AkiButtonSize.Large, fullWidth = true) { onDismiss(); apply() }
            AkiButton("Reset", variant = AkiButtonVariant.Ghost, fullWidth = true) { onChoices(emptyMap()); onDismiss(); apply() }
        }
    }
}
