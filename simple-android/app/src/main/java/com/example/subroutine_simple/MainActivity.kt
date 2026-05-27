package com.example.subroutine_simple

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.FormatListBulleted
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.outlined.FormatListBulleted
import androidx.compose.material.icons.outlined.Inbox
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberTopAppBarState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation3.runtime.entryProvider
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import com.example.subroutine_simple.ui.MainViewModel
import com.example.subroutine_simple.ui.components.CreateActionSheet
import com.example.subroutine_simple.ui.screens.BacklogScreen
import com.example.subroutine_simple.ui.screens.EditActionScreen
import com.example.subroutine_simple.ui.screens.EditEventScreen
import com.example.subroutine_simple.ui.screens.QueueScreen
import com.example.subroutine_simple.ui.theme.SubroutineSimpleTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            SubroutineSimpleTheme {
                AppNavigation()
            }
        }
    }
}

@Composable
private fun AppNavigation(
    viewModel: MainViewModel = viewModel(),
) {
    val backStack = rememberNavBackStack(MainRoute)

    NavDisplay(
        backStack = backStack,
        onBack = { backStack.removeLastOrNull() },
        transitionSpec = {
            slideInHorizontally(initialOffsetX = { it }) togetherWith
                slideOutHorizontally(targetOffsetX = { -it })
        },
        popTransitionSpec = {
            slideInHorizontally(initialOffsetX = { -it }) togetherWith
                slideOutHorizontally(targetOffsetX = { it })
        },
        predictivePopTransitionSpec = {
            slideInHorizontally(initialOffsetX = { -it }) togetherWith
                slideOutHorizontally(targetOffsetX = { it })
        },
        entryProvider = entryProvider {
            entry<MainRoute> {
                MainScreen(
                    viewModel = viewModel,
                    onEditAction = { actionId -> backStack.add(EditActionRoute(actionId)) },
                    onEditEvent = { eventId -> backStack.add(EditEventRoute(eventId)) },
                )
            }
            entry<EditActionRoute> { route ->
                EditActionScreen(
                    actionId = route.actionId,
                    viewModel = viewModel,
                    onBack = { backStack.removeLastOrNull() },
                )
            }
            entry<EditEventRoute> { route ->
                EditEventScreen(
                    eventId = route.eventId,
                    viewModel = viewModel,
                    onBack = { backStack.removeLastOrNull() },
                )
            }
        },
    )
}

@Composable
private fun MainScreen(
    viewModel: MainViewModel,
    onEditAction: (actionId: String) -> Unit,
    onEditEvent: (eventId: String) -> Unit,
) {
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior(
        rememberTopAppBarState()
    )
    var selectedTab by remember { mutableIntStateOf(0) }
    val tabs = listOf("Queue", "Backlog")
    val showCreateSheet by viewModel.showCreateSheet.collectAsStateWithLifecycle()

    Scaffold(
        modifier = Modifier
            .fillMaxSize()
            .nestedScroll(scrollBehavior.nestedScrollConnection),
        topBar = {
            LargeTopAppBar(
                title = { Text(tabs[selectedTab]) },
                scrollBehavior = scrollBehavior,
            )
        },
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = selectedTab == 0,
                    onClick = { selectedTab = 0 },
                    icon = {
                        Icon(
                            if (selectedTab == 0) Icons.Filled.FormatListBulleted
                            else Icons.Outlined.FormatListBulleted,
                            contentDescription = "Queue",
                        )
                    },
                    label = { Text("Queue") },
                )
                NavigationBarItem(
                    selected = selectedTab == 1,
                    onClick = { selectedTab = 1 },
                    icon = {
                        Icon(
                            if (selectedTab == 1) Icons.Filled.Inbox
                            else Icons.Outlined.Inbox,
                            contentDescription = "Backlog",
                        )
                    },
                    label = { Text("Backlog") },
                )
            }
        },
        floatingActionButton = {
            FloatingActionButton(
                onClick = viewModel::openCreateSheet,
                shape = FloatingActionButtonDefaults.shape,
            ) {
                Icon(Icons.Filled.Add, contentDescription = "New action")
            }
        },
    ) { innerPadding ->
        when (selectedTab) {
            0 -> QueueScreen(
                viewModel = viewModel,
                contentPadding = innerPadding,
                onEditAction = onEditAction,
                onEditEvent = onEditEvent,
                modifier = Modifier.fillMaxSize(),
            )
            1 -> BacklogScreen(
                viewModel = viewModel,
                contentPadding = innerPadding,
                onEditAction = onEditAction,
                modifier = Modifier.fillMaxSize(),
            )
        }
    }

    if (showCreateSheet) {
        CreateActionSheet(
            onDismiss = viewModel::closeCreateSheet,
            onConfirm = viewModel::createAction,
        )
    }
}
