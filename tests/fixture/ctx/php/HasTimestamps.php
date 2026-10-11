<?php

namespace App\Services;

trait HasTimestamps
{
    public function touch(): void
    {
        $this->updatedAt = time();
    }
}
