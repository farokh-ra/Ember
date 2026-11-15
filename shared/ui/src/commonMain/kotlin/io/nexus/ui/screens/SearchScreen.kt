package io.nexus.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import io.nexus.presentation.PreviewNetworkState
import io.nexus.presentation.PreviewScenario
import io.nexus.presentation.content.previewContentState
import io.nexus.presentation.demo.DemoCatalog
import io.nexus.presentation.demo.DemoCategory
import io.nexus.ui.components.ContentStateHost
import io.nexus.ui.components.NexusChoiceChip
import io.nexus.ui.components.NexusGroupLabel
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusInlineEmpty
import io.nexus.ui.components.NexusPrimaryButton
import io.nexus.ui.components.NexusScreen
import io.nexus.ui.components.NexusSectionHeader
import io.nexus.ui.components.PreviewStateBar
import io.nexus.ui.components.PublicationCard
import io.nexus.ui.components.StatusTone
import io.nexus.ui.components.demoCategoryLabel
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/** Local and discovered metadata search prototype. */
@Composable
public fun SearchScreen(
    scenario: PreviewScenario,
    network: PreviewNetworkState,
    onScenarioChange: (PreviewScenario) -> Unit,
    onNetworkChange: (PreviewNetworkState) -> Unit,
    onOpenFile: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    val focusManager = LocalFocusManager.current
    var queryText by remember { mutableStateOf("") }
    var activeQuery by remember { mutableStateOf("") }
    var category by remember { mutableStateOf<DemoCategory?>(null) }

    val results = remember(activeQuery, category) { DemoCatalog.search(activeQuery, category) }
    val content = previewContentState(scenario) { results }

    fun submitSearch() {
        activeQuery = queryText
        focusManager.clearFocus()
    }

    fun clearFilters() {
        queryText = ""
        activeQuery = ""
        category = null
        focusManager.clearFocus()
    }

    NexusScreen(
        modifier = modifier,
        title = strings.searchTitle,
        subtitle = strings.searchSubtitle,
        previewControls = {
            PreviewStateBar(
                scenario = scenario,
                onScenarioChange = onScenarioChange,
                network = network,
                onNetworkChange = onNetworkChange,
            )
        },
    ) {
        NexusInfoBanner(
            title = strings.demoContent,
            body = strings.searchCoverage,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Info,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                modifier = Modifier.weight(1f),
                value = queryText,
                onValueChange = { queryText = it },
                label = { Text(strings.searchTitle) },
                placeholder = { Text(strings.searchPlaceholder) },
                singleLine = true,
                leadingIcon = {
                    Icon(
                        imageVector = NexusIcons.Search,
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                    )
                },
                trailingIcon = {
                    if (queryText.isNotEmpty()) {
                        IconButton(
                            modifier = Modifier.size(NexusSizes.minimumTouchTarget),
                            onClick = { clearFilters() },
                        ) {
                            Icon(
                                imageVector = NexusIcons.Close,
                                contentDescription = strings.searchClear,
                                modifier = Modifier.size(20.dp),
                            )
                        }
                    }
                },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { submitSearch() }),
            )
            NexusPrimaryButton(
                text = strings.searchAction,
                onClick = { submitSearch() },
                icon = NexusIcons.Search,
            )
        }
        NexusGroupLabel(text = strings.exploreCategories)
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState())
                    .selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(NexusSpacing.sm),
        ) {
            NexusChoiceChip(
                text = strings.categoryAll,
                selected = category == null,
                onClick = { category = null },
            )
            DemoCategory.entries.forEach { option ->
                NexusChoiceChip(
                    text = demoCategoryLabel(option, strings),
                    selected = category == option,
                    onClick = { category = option },
                )
            }
        }
        ContentStateHost(
            state = content,
            onShowData = { onScenarioChange(PreviewScenario.READY) },
            emptyTitle = strings.emptyTitle,
            emptyBody = strings.emptyBody,
        ) { visible ->
            when {
                activeQuery.isBlank() -> {
                    NexusInlineEmpty(
                        title = strings.searchBlankTitle,
                        body = strings.searchBlankBody,
                    )
                }

                visible.isEmpty() -> {
                    NexusInlineEmpty(
                        title = strings.searchNoResultsTitle,
                        body = strings.searchNoResultsBody,
                        actionLabel = strings.searchClearFilters,
                        onAction = { clearFilters() },
                    )
                }

                else -> {
                    Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.sm)) {
                        NexusSectionHeader(
                            title = strings.searchResultCount(visible.size),
                            subtitle = strings.searchPartial,
                        )
                        visible.forEach { publication ->
                            PublicationCard(
                                publication = publication,
                                onClick = { onOpenFile(publication.recordId) },
                            )
                        }
                    }
                }
            }
        }
    }
}
