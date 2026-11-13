package io.nexus.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.unit.dp
import io.nexus.presentation.identity.BioError
import io.nexus.presentation.identity.DisplayNameError
import io.nexus.presentation.identity.IdentityDraft
import io.nexus.ui.components.NexusBackButton
import io.nexus.ui.components.NexusDemoBadge
import io.nexus.ui.components.NexusDivider
import io.nexus.ui.components.NexusInfoBanner
import io.nexus.ui.components.NexusPrimaryButton
import io.nexus.ui.components.NexusWordmark
import io.nexus.ui.components.StatusTone
import io.nexus.ui.icons.NexusIcons
import io.nexus.ui.resources.LocalNexusStrings
import io.nexus.ui.resources.NexusStrings
import io.nexus.ui.theme.NexusBreakpoints
import io.nexus.ui.theme.NexusSizes
import io.nexus.ui.theme.NexusSpacing

/**
 * Local identity setup prototype.
 *
 * Validation is real; key generation and storage are not. The screen says so
 * repeatedly so a demo profile is never mistaken for a persisted identity.
 */
@Composable
public fun IdentitySetupScreen(
    onContinue: (String) -> Unit,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    var draft by remember { mutableStateOf(IdentityDraft()) }
    var attempted by remember { mutableStateOf(false) }
    val focusRequester = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current

    val nameError = draft.displayNameError.takeIf { attempted }
    val bioError = draft.bioError.takeIf { attempted }

    BoxWithConstraints(
        modifier =
            modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
    ) {
        val expanded = maxWidth >= NexusBreakpoints.expanded
        val gutter = if (expanded) NexusSpacing.xxxl else NexusSpacing.lg
        Column(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .widthIn(max = NexusSizes.maxContentWidth)
                    .padding(horizontal = gutter, vertical = NexusSpacing.xl),
            verticalArrangement = Arrangement.spacedBy(NexusSpacing.xl),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                NexusWordmark(showTagline = true)
                Spacer(modifier = Modifier.weight(1f))
                NexusBackButton(onBack = onBack)
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = strings.identityTitle,
                    modifier = Modifier.weight(1f),
                    style =
                        if (expanded) {
                            MaterialTheme.typography.headlineLarge
                        } else {
                            MaterialTheme.typography.headlineSmall
                        },
                    color = MaterialTheme.colorScheme.onSurface,
                )
                NexusDemoBadge()
            }
            Text(
                text = strings.identityBody,
                modifier = Modifier.widthIn(max = 680.dp),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            NexusInfoBanner(
                title = strings.demoContent,
                body = strings.identityDemoNote,
                tone = StatusTone.CAUTION,
                icon = NexusIcons.Info,
            )
            if (expanded) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(NexusSpacing.xl),
                    verticalAlignment = Alignment.Top,
                ) {
                    IdentityForm(
                        modifier = Modifier.weight(1f),
                        draft = draft,
                        onDraftChange = { draft = it },
                        nameError = nameError,
                        bioError = bioError,
                        focusRequester = focusRequester,
                        onContinue = {
                            attempted = true
                            if (draft.canContinue) {
                                focusManager.clearFocus()
                                onContinue(draft.trimmedDisplayName)
                            } else {
                                focusRequester.requestFocus()
                            }
                        },
                    )
                    Column(
                        modifier = Modifier.widthIn(max = 380.dp),
                        verticalArrangement = Arrangement.spacedBy(NexusSpacing.md),
                    ) {
                        IdentityNoticeCards()
                    }
                }
            } else {
                IdentityForm(
                    modifier = Modifier.fillMaxWidth(),
                    draft = draft,
                    onDraftChange = { draft = it },
                    nameError = nameError,
                    bioError = bioError,
                    focusRequester = focusRequester,
                    onContinue = {
                        attempted = true
                        if (draft.canContinue) {
                            focusManager.clearFocus()
                            onContinue(draft.trimmedDisplayName)
                        } else {
                            focusRequester.requestFocus()
                        }
                    },
                )
                IdentityNoticeCards()
            }
        }
    }
}

@Composable
private fun IdentityForm(
    draft: IdentityDraft,
    onDraftChange: (IdentityDraft) -> Unit,
    nameError: DisplayNameError?,
    bioError: BioError?,
    focusRequester: FocusRequester,
    onContinue: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val strings = LocalNexusStrings.current
    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(NexusSpacing.lg),
    ) {
        Text(
            text = strings.demoContent,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedTextField(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
            value = draft.displayName,
            onValueChange = { onDraftChange(draft.copy(displayName = it)) },
            label = { Text(strings.identityNameLabel) },
            placeholder = { Text(strings.identityNamePlaceholder) },
            singleLine = true,
            isError = nameError != null,
            supportingText = {
                Text(
                    text =
                        nameError?.let { displayNameErrorText(it, strings) }
                            ?: strings.identityNameCounter(
                                draft.displayName.length,
                                IdentityDraft.MAX_NAME_LENGTH,
                            ),
                )
            },
        )
        OutlinedTextField(
            modifier = Modifier.fillMaxWidth(),
            value = draft.bio,
            onValueChange = { onDraftChange(draft.copy(bio = it)) },
            label = { Text(strings.identityBioLabel) },
            placeholder = { Text(strings.identityBioPlaceholder) },
            minLines = 3,
            maxLines = 5,
            isError = bioError != null,
            supportingText = {
                Text(
                    text =
                        bioError?.let { bioErrorText(it, strings) }
                            ?: strings.identityBioCounter(
                                draft.bio.length,
                                IdentityDraft.MAX_BIO_LENGTH,
                            ),
                )
            },
        )
        NexusPrimaryButton(
            modifier = Modifier.fillMaxWidth(),
            text = strings.identityContinue,
            onClick = onContinue,
            icon = NexusIcons.ChevronRight,
        )
        Text(
            text = strings.identityDemoNote,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun IdentityNoticeCards() {
    val strings = LocalNexusStrings.current
    Column(verticalArrangement = Arrangement.spacedBy(NexusSpacing.md)) {
        NexusInfoBanner(
            title = strings.identityPublicTitle,
            body = strings.identityPublicBody,
            tone = StatusTone.INFO,
            icon = NexusIcons.Globe,
        )
        NexusInfoBanner(
            title = strings.identityNoRecoveryTitle,
            body = strings.identityNoRecoveryBody,
            tone = StatusTone.CAUTION,
            icon = NexusIcons.Warning,
        )
        NexusDivider()
        Text(
            text = strings.profileUsernameNote,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

private fun displayNameErrorText(
    error: DisplayNameError,
    strings: NexusStrings,
): String =
    when (error) {
        DisplayNameError.EMPTY -> strings.identityNameErrorEmpty
        DisplayNameError.TOO_SHORT -> strings.identityNameErrorTooShort
        DisplayNameError.TOO_LONG -> strings.identityNameErrorTooLong
    }

private fun bioErrorText(
    error: BioError,
    strings: NexusStrings,
): String =
    when (error) {
        BioError.TOO_LONG -> strings.identityBioErrorTooLong
    }
