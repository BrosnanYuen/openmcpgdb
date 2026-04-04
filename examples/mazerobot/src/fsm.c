#include "fsm.h"
#include <stdio.h>
#include <stdlib.h>

static FsmState state_idle;
static FsmState state_explore;
static FsmState state_move;
static FsmState state_backtrack;
static FsmState state_goal;

static void init_states(void) {
    static int initialized = 0;
    if (initialized) return;

    state_idle.id = STATE_IDLE;
    state_idle.on_enter = fsm_state_idle_enter;
    state_idle.on_exit = fsm_state_idle_exit;
    state_idle.on_update = fsm_state_idle_update;

    state_explore.id = STATE_EXPLORE;
    state_explore.on_enter = fsm_state_explore_enter;
    state_explore.on_exit = fsm_state_explore_exit;
    state_explore.on_update = fsm_state_explore_update;

    state_move.id = STATE_MOVE;
    state_move.on_enter = fsm_state_move_enter;
    state_move.on_exit = fsm_state_move_exit;
    state_move.on_update = fsm_state_move_update;

    state_backtrack.id = STATE_BACKTRACK;
    state_backtrack.on_enter = fsm_state_backtrack_enter;
    state_backtrack.on_exit = fsm_state_backtrack_exit;
    state_backtrack.on_update = fsm_state_backtrack_update;

    state_goal.id = STATE_GOAL_REACHED;
    state_goal.on_enter = fsm_state_goal_enter;
    state_goal.on_exit = fsm_state_goal_exit;
    state_goal.on_update = fsm_state_goal_update;

    initialized = 1;
}

void fsm_init(Fsm *fsm, FsmState *initial, void *ctx) {
    init_states();
    fsm->current = initial;
    fsm->ctx = ctx;
    if (fsm->current->on_enter) {
        fsm->current->on_enter(fsm->ctx);
    }
}

void fsm_run(Fsm *fsm) {
    if (!fsm->current) return;

    FsmState *next = fsm->current->on_update(fsm->ctx);

    if (next && next != fsm->current) {
        if (fsm->current->on_exit) {
            fsm->current->on_exit(fsm->ctx);
        }
        fsm->current = next;
        if (fsm->current->on_enter) {
            fsm->current->on_enter(fsm->ctx);
        }
    }
}

void fsm_set_state(Fsm *fsm, FsmState *next) {
    if (!next || next == fsm->current) return;

    if (fsm->current->on_exit) {
        fsm->current->on_exit(fsm->ctx);
    }
    fsm->current = next;
    if (fsm->current->on_enter) {
        fsm->current->on_enter(fsm->ctx);
    }
}

void fsm_state_idle_enter(void *ctx) {
    Robot *robot = (Robot *)ctx;
    printf("[FSM] Entering IDLE state\n");
    robot_set_state(robot, STATE_IDLE);
}

void fsm_state_explore_enter(void *ctx) {
    Robot *robot = (Robot *)ctx;
    printf("[FSM] Entering EXPLORE state\n");
    robot_set_state(robot, STATE_EXPLORE);
    robot_mark_visited(robot, robot->sim->robot_row, robot->sim->robot_col);
}

void fsm_state_move_enter(void *ctx) {
    Robot *robot = (Robot *)ctx;
    printf("[FSM] Entering MOVE state\n");
    robot_set_state(robot, STATE_MOVE);
}

void fsm_state_backtrack_enter(void *ctx) {
    Robot *robot = (Robot *)ctx;
    printf("[FSM] Entering BACKTRACK state\n");
    robot_set_state(robot, STATE_BACKTRACK);
}

void fsm_state_goal_enter(void *ctx) {
    Robot *robot = (Robot *)ctx;
    printf("[FSM] Entering GOAL_REACHED state\n");
    robot_set_state(robot, STATE_GOAL_REACHED);
    simulator_print_status(robot->sim);
}

FsmState* fsm_state_idle_update(void *ctx) {
    (void)ctx;
    return fsm_get_state_explore();
}

FsmState* fsm_state_explore_update(void *ctx) {
    Robot *robot = (Robot *)ctx;

    if (simulator_is_at_goal(robot->sim)) {
        return fsm_get_state_goal();
    }

    Direction dirs[] = {DIR_NORTH, DIR_EAST, DIR_SOUTH, DIR_WEST};

    for (int i = 0; i < 4; i++) {
        int nr = robot->sim->robot_row + robot_get_row_offset(dirs[i]);
        int nc = robot->sim->robot_col + robot_get_col_offset(dirs[i]);

        if (robot_can_move(robot, nr, nc) && !robot_is_visited(robot, nr, nc)) {
            robot->facing = dirs[i];
            Position pos;
            pos.row = nr;
            pos.col = nc;
            robot->path[robot->path_length++] = pos;
            return fsm_get_state_move();
        }
    }

    if (robot->path_length > 0) {
        return fsm_get_state_backtrack();
    }

    return NULL;
}

FsmState* fsm_state_move_update(void *ctx) {
    Robot *robot = (Robot *)ctx;

    int nr = robot->sim->robot_row + robot_get_row_offset(robot->facing);
    int nc = robot->sim->robot_col + robot_get_col_offset(robot->facing);

    if (simulator_move_robot(robot->sim, nr, nc)) {
        robot_mark_visited(robot, nr, nc);
    }

    if (simulator_is_at_goal(robot->sim)) {
        return fsm_get_state_goal();
    }

    return fsm_get_state_explore();
}

FsmState* fsm_state_backtrack_update(void *ctx) {
    Robot *robot = (Robot *)ctx;

    if (robot->path_length <= 0) {
        return fsm_get_state_explore();
    }

    robot->path_length--;
    Position prev = robot->path[robot->path_length];

    simulator_move_robot(robot->sim, prev.row, prev.col);

    return fsm_get_state_explore();
}

FsmState* fsm_state_goal_update(void *ctx) {
    (void)ctx;
    return NULL;
}

void fsm_state_idle_exit(void *ctx) {
    (void)ctx;
}

void fsm_state_explore_exit(void *ctx) {
    (void)ctx;
}

void fsm_state_move_exit(void *ctx) {
    (void)ctx;
}

void fsm_state_backtrack_exit(void *ctx) {
    (void)ctx;
}

void fsm_state_goal_exit(void *ctx) {
    (void)ctx;
}

FsmState* fsm_get_state_idle(void) {
    init_states();
    return &state_idle;
}

FsmState* fsm_get_state_explore(void) {
    init_states();
    return &state_explore;
}

FsmState* fsm_get_state_move(void) {
    init_states();
    return &state_move;
}

FsmState* fsm_get_state_backtrack(void) {
    init_states();
    return &state_backtrack;
}

FsmState* fsm_get_state_goal(void) {
    init_states();
    return &state_goal;
}
