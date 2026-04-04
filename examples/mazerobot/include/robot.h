#ifndef ROBOT_H
#define ROBOT_H

#include "simulator.h"

typedef enum {
    DIR_NORTH = 0,
    DIR_EAST = 1,
    DIR_SOUTH = 2,
    DIR_WEST = 3
} Direction;

typedef enum {
    STATE_IDLE = 0,
    STATE_EXPLORE = 1,
    STATE_MOVE = 2,
    STATE_BACKTRACK = 3,
    STATE_GOAL_REACHED = 4
} RobotState;

typedef struct {
    int row;
    int col;
} Position;

typedef struct {
    Simulator *sim;
    RobotState state;
    Direction facing;
    Position path[MAZE_MAX_SIZE * MAZE_MAX_SIZE];
    int path_length;
    int visited[MAZE_MAX_SIZE][MAZE_MAX_SIZE];
} Robot;

void robot_init(Robot *robot, Simulator *sim);
RobotState robot_get_state(const Robot *robot);
void robot_set_state(Robot *robot, RobotState state);
Position robot_get_next_position(const Robot *robot);
bool robot_can_move(const Robot *robot, int row, int col);
void robot_mark_visited(Robot *robot, int row, int col);
bool robot_is_visited(const Robot *robot, int row, int col);
Direction robot_turn_left(Direction dir);
Direction robot_turn_right(Direction dir);
int robot_get_row_offset(Direction dir);
int robot_get_col_offset(Direction dir);

#endif
