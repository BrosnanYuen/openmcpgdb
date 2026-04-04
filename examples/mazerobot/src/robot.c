#include "robot.h"
#include <string.h>

void robot_init(Robot *robot, Simulator *sim) {
    robot->sim = sim;
    robot->state = STATE_IDLE;
    robot->facing = DIR_EAST;
    robot->path_length = 0;
    memset(robot->visited, 0, sizeof(robot->visited));
}

RobotState robot_get_state(const Robot *robot) {
    return robot->state;
}

void robot_set_state(Robot *robot, RobotState state) {
    robot->state = state;
}

Position robot_get_next_position(const Robot *robot) {
    Position pos;
    pos.row = robot->sim->robot_row + robot_get_row_offset(robot->facing);
    pos.col = robot->sim->robot_col + robot_get_col_offset(robot->facing);
    return pos;
}

bool robot_can_move(const Robot *robot, int row, int col) {
    return maze_is_valid_move(&robot->sim->maze, row, col);
}

void robot_mark_visited(Robot *robot, int row, int col) {
    robot->visited[row][col] = 1;
}

bool robot_is_visited(const Robot *robot, int row, int col) {
    return robot->visited[row][col] == 1;
}

Direction robot_turn_left(Direction dir) {
    return (Direction)((dir + 3) % 4);
}

Direction robot_turn_right(Direction dir) {
    return (Direction)((dir + 1) % 4);
}

int robot_get_row_offset(Direction dir) {
    static const int offsets[] = {-1, 0, 1, 0};
    return offsets[dir];
}

int robot_get_col_offset(Direction dir) {
    static const int offsets[] = {0, 1, 0, -1};
    return offsets[dir];
}
