#ifndef FSM_H
#define FSM_H

#include "robot.h"

typedef struct FsmState FsmState;

typedef void (*FsmEnterFunc)(void *ctx);
typedef void (*FsmExitFunc)(void *ctx);
typedef FsmState* (*FsmUpdateFunc)(void *ctx);

struct FsmState {
    RobotState id;
    FsmEnterFunc on_enter;
    FsmExitFunc on_exit;
    FsmUpdateFunc on_update;
};

typedef struct {
    FsmState *current;
    void *ctx;
} Fsm;

void fsm_init(Fsm *fsm, FsmState *initial, void *ctx);
void fsm_run(Fsm *fsm);
void fsm_set_state(Fsm *fsm, FsmState *next);

void fsm_state_idle_enter(void *ctx);
void fsm_state_explore_enter(void *ctx);
void fsm_state_move_enter(void *ctx);
void fsm_state_backtrack_enter(void *ctx);
void fsm_state_goal_enter(void *ctx);

FsmState* fsm_state_idle_update(void *ctx);
FsmState* fsm_state_explore_update(void *ctx);
FsmState* fsm_state_move_update(void *ctx);
FsmState* fsm_state_backtrack_update(void *ctx);
FsmState* fsm_state_goal_update(void *ctx);

void fsm_state_idle_exit(void *ctx);
void fsm_state_explore_exit(void *ctx);
void fsm_state_move_exit(void *ctx);
void fsm_state_backtrack_exit(void *ctx);
void fsm_state_goal_exit(void *ctx);

FsmState* fsm_get_state_idle(void);
FsmState* fsm_get_state_explore(void);
FsmState* fsm_get_state_move(void);
FsmState* fsm_get_state_backtrack(void);
FsmState* fsm_get_state_goal(void);

#endif
