package com.example.subroutine_simple.ui.screens

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.LoadingIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.pulltorefresh.rememberPullToRefreshState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.example.subroutine_simple.ui.ActionsUiState
import com.example.subroutine_simple.ui.MainViewModel
import com.example.subroutine_simple.ui.components.ActionListItem

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun BacklogScreen(
    viewModel: MainViewModel,
    contentPadding: PaddingValues,
    onEditAction: (actionId: String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val uiState by viewModel.uiState.collectAsStateWithLifecycle()
    val completing by viewModel.completing.collectAsStateWithLifecycle()
    val pullState = rememberPullToRefreshState()

    PullToRefreshBox(
        isRefreshing = uiState is ActionsUiState.Loading,
        onRefresh = viewModel::loadActions,
        state = pullState,
        modifier = modifier.fillMaxSize(),
    ) {
        when (val state = uiState) {
            is ActionsUiState.Loading -> {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    LoadingIndicator()
                }
            }
            is ActionsUiState.Error -> {
                Box(
                    Modifier
                        .fillMaxSize()
                        .padding(contentPadding),
                    contentAlignment = Alignment.Center,
                ) {
                    Text("Error: ${state.message}")
                }
            }
            is ActionsUiState.Success -> {
                val listState = rememberLazyListState()
                LazyColumn(
                    state = listState,
                    contentPadding = contentPadding,
                    modifier = Modifier.fillMaxSize(),
                ) {
                    items(
                        items = state.backlogged,
                        key = { it.id },
                        contentType = { "backlogged_action" },
                    ) { action ->
                        ActionListItem(
                            action = action,
                            isCompleting = action.id in completing,
                            onComplete = { viewModel.completeAction(action.id) },
                            onClick = { onEditAction(action.id) },
                        )
                    }
                    if (state.backlogged.isEmpty()) {
                        item {
                            Box(
                                Modifier
                                    .fillMaxSize()
                                    .padding(32.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text("Backlog is empty")
                            }
                        }
                    }
                }
            }
        }
    }
}
